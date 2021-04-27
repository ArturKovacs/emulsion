use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

use gelatin::image::{self, gif::GifDecoder, png::PngDecoder, AnimationDecoder, ImageFormat};

pub mod errors {
	use gelatin::glium::texture;
	use gelatin::image;
	use std::io;

	error_chain! {
		foreign_links {
			Io(io::Error) #[doc = "Error during IO"];
			TextureCreationError(texture::TextureCreationError);
			ImageLoadError(image::ImageError);
			ExifError(exif::Error);
			SvgError(usvg::Error);
			AvifError(libavif_image::Error) #[cfg(feature = "avif")];
		}
	}
}

use self::errors::*;

/// We want to prevent prefetch operations taking place when the target image is not yet loaded.
/// To implement this we define a variable that is read by the loader threads and
/// which will only carry out the request if the focused request id matches their request or
/// if the focused is set to `NON_EXISTENT_REQUEST_ID`
pub static PRIORITY_REQUEST_ID: AtomicU32 = AtomicU32::new(0); // The first request usually
pub const NON_EXISTENT_REQUEST_ID: u32 = std::u32::MAX;

pub enum ImgFormat {
	Image(ImageFormat),
	Svg,
	#[cfg(feature = "avif")]
	Avif,
}

/// These values define the transformation for a pixel array which is to be displayed.
///
/// The default case is when the 0th row is at the top and the 0th column is at the left side of the
/// image. This is represented by the value `Deg0`. All other cases must be interpreted as relative
/// to this. The rotation part is counter-clockwise. When there's a flip it's always interpreted as
/// if it happened after the rotation.
#[derive(Debug, Copy, Clone)]
pub enum Orientation {
	/// Exif 1
	Deg0,

	/// Exif 2
	Deg0HorFlip,

	/// Exif 3
	Deg180,

	/// Exif 4
	Deg180HorFlip,

	/// Exif 5
	Deg90VerFlip,

	/// Exif 6
	Deg270,

	/// Exif 7
	Deg270VerFlip,

	/// Exif 8
	Deg90,
}
impl Default for Orientation {
	fn default() -> Self {
		Orientation::Deg0
	}
}

/// Detects the format of an image file. It looks at the first 512 bytes;
/// if that fails, it uses the file ending.
pub fn detect_format(path: &Path) -> Result<ImgFormat> {
	let mut file = fs::File::open(path)?;
	let mut file_start_bytes = [0; 512];

	// Try to detect the format from the first 512 bytes
	if file.read_exact(&mut file_start_bytes).is_ok() {
		#[cfg(feature = "avif")]
		{
			if libavif_image::is_avif(&file_start_bytes) {
				return Ok(ImgFormat::Avif);
			}
		}
		if path.extension() == Some(std::ffi::OsStr::new("svg")) {
			return Ok(ImgFormat::Svg);
		}
		if let Ok(format) = image::guess_format(&file_start_bytes) {
			return Ok(ImgFormat::Image(format));
		}
	}

	// If that didn't work, try to detect the format from the file ending
	Ok(ImgFormat::Image(ImageFormat::from_path(path)?))
}

pub fn detect_orientation(path: &Path) -> Result<Orientation> {
	let file = std::fs::File::open(path)?;
	let mut bufreader = std::io::BufReader::new(&file);
	let exifreader = exif::Reader::new();
	let exif = exifreader.read_from_container(&mut bufreader)?;
	if let Some(orientation) = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
		if let exif::Value::Short(ref shorts) = orientation.value {
			if let Some(&exif_orientation) = shorts.get(0) {
				// According to page 30 of http://www.cipa.jp/std/documents/e/DC-008-2012_E.pdf
				match exif_orientation {
					1 => Ok(Orientation::Deg0),
					2 => Ok(Orientation::Deg0HorFlip),
					3 => Ok(Orientation::Deg180),
					4 => Ok(Orientation::Deg180HorFlip),
					5 => Ok(Orientation::Deg90VerFlip),
					6 => Ok(Orientation::Deg270),
					7 => Ok(Orientation::Deg270VerFlip),
					8 => Ok(Orientation::Deg90),
					_ => {
						eprintln!("Invalid Exif orientation. Using default orientation.");
						Ok(Orientation::Deg0)
					}
				}
			} else {
				Ok(Orientation::Deg0)
			}
		} else {
			Err("EXIF orientation was expected to be of type 'short' but it wasn't".into())
		}
	} else {
		Ok(Orientation::Deg0)
	}
}

pub fn simple_load_image(path: &Path, image_format: ImageFormat) -> Result<image::RgbaImage> {
	let reader = BufReader::new(fs::File::open(path)?);
	Ok(image::load(reader, image_format)?.into_rgba8())
}

/// Returns an iterator over the animation frames of a GIF file
pub fn load_gif(path: &Path, req_id: u32) -> Result<impl Iterator<Item = Result<LoadResult>>> {
	let file = fs::File::open(path)?;
	let decoder = GifDecoder::new(file)?;
	Ok(load_animation(req_id, decoder))
}

/// Parse, render and gather an SVG into a ImageBuffer<Rgba>
pub fn load_svg(path: &std::path::Path) -> Result<image::RgbaImage> {
	let opt = usvg::Options::default();
	let rtree = usvg::Tree::from_file(path, &opt)?;
	let size = rtree.svg_node().size;
	let (width, height) = (size.width(), size.height());
	// Scale to fit 4096
	let zoom = 4096. / width.max(height);
	let (width, height) = ((width * zoom) as u32, (height * zoom) as u32);
	// These unwrapped Options are fine as long as the dimensions are correct
	let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
	resvg::render(&rtree, usvg::FitTo::Zoom(zoom as f32), pixmap.as_mut()).unwrap();
	Ok(image::RgbaImage::from_raw(width, height, pixmap.take()).unwrap())
}

pub fn complex_load_image<F>(
	path: &Path,
	allow_animation: bool,
	req_id: u32,
	mut process_image: F,
) -> Result<()>
where
	F: FnMut(LoadResult) -> Result<()>,
{
	let image_format = detect_format(path)?;
	let orientation = detect_orientation(path).unwrap_or(Orientation::Deg0);

	match image_format {
		ImgFormat::Image(ImageFormat::Gif) => {
			let mut frames = load_gif(path, req_id)?;
			if allow_animation {
				for frame in frames {
					process_image(frame?)?;
				}
			} else if let Some(frame) = frames.next() {
				process_image(frame?)?;
			}
		}
		ImgFormat::Image(ImageFormat::Png) => {
			let file = fs::File::open(path)?;
			let decoder = PngDecoder::new(file)?;
			if decoder.is_apng() {
				let mut animation = load_animation(req_id, decoder.apng());
				if allow_animation {
					for frame in animation {
						process_image(frame?)?;
					}
				} else if let Some(frame) = animation.next() {
					process_image(frame?)?;
				}
			} else {
				let image = simple_load_image(path, ImageFormat::Png)?;
				process_image(LoadResult::Frame { req_id, image, delay_nano: 0, orientation })?;
			}
		}
		ImgFormat::Image(image_format) => {
			let image = simple_load_image(path, image_format)?;
			process_image(LoadResult::Frame { req_id, image, delay_nano: 0, orientation })?;
		}
		#[cfg(feature = "avif")]
		ImgFormat::Avif => {
			let buf = fs::read(path)?;
			let image = libavif_image::read(&buf)?.into_rgba8();
			process_image(LoadResult::Frame { req_id, image, delay_nano: 0, orientation })?;
		}
		ImgFormat::Svg => {
			let image = load_svg(path)?;
			process_image(LoadResult::Frame { req_id, image, delay_nano: 0, orientation })?;
		}
	}

	Ok(())
}

fn load_animation(
	req_id: u32,
	decoder: impl AnimationDecoder<'static>,
) -> impl Iterator<Item = Result<LoadResult>> {
	let frames = decoder.into_frames();

	frames.map(move |frame| {
		Ok(frame.map(|frame| {
			let (mut numerator_ms, mut denom) = frame.delay().numer_denom_ms();
			if numerator_ms == 0 {
				// Some animated gifs specify a 0 inter-frame delay, but
				// most gif viewers interpret this as a 0.1 second delay.
				numerator_ms = 100;
				denom = 1;
			}
			let numerator_nano = numerator_ms as u64 * 1_000_000;
			let delay_nano = numerator_nano / (denom as u64);
			let image = frame.into_buffer();
			LoadResult::Frame { req_id, image, delay_nano, orientation: Orientation::Deg0 }
		})?)
	})
}

pub fn is_file_supported(filename: &Path) -> bool {
	if let Some(ext) = filename.extension() {
		if let Some(ext) = ext.to_str() {
			let ext = ext.to_lowercase();
			match ext.as_str() {
				"jpg" | "jpeg" | "png" | "apng" | "gif" | "webp" | "tif" | "tiff" | "tga"
				| "bmp" | "ico" | "hdr" | "pbm" | "pam" | "ppm" | "pgm" => {
					return true;
				}
				#[cfg(feature = "avif")]
				"avif" => return true,
				_ => (),
			}
		}
	}
	detect_format(filename).is_ok()
}

#[derive(Debug, Clone)]
pub struct LoadRequest {
	pub req_id: u32,
	pub path: PathBuf,
}

pub enum LoadResult {
	Start {
		req_id: u32,
		metadata: fs::Metadata,
	},
	Frame {
		req_id: u32,
		image: image::RgbaImage,
		delay_nano: u64,

		/// How much does the image need to be rotated counter-clockwise to be shown correctly
		orientation: Orientation,
	},
	Done {
		req_id: u32,
	},
	Failed {
		req_id: u32,
	},
}

impl LoadResult {
	pub fn req_id(&self) -> u32 {
		match self {
			LoadResult::Start { req_id, .. } => *req_id,
			LoadResult::Frame { req_id, .. } => *req_id,
			LoadResult::Done { req_id, .. } => *req_id,
			LoadResult::Failed { req_id, .. } => *req_id,
		}
	}
}

pub struct ImageLoader {
	running: Arc<AtomicBool>,
	join_handles: Option<Vec<thread::JoinHandle<()>>>,
	image_rx: Receiver<LoadResult>,
	path_tx: Sender<LoadRequest>,
}

impl ImageLoader {
	/// # Arguemnts
	/// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
	pub fn new(threads: u32) -> ImageLoader {
		let running = Arc::new(AtomicBool::from(true));
		let (load_request_tx, load_request_rx) = channel();
		let load_request_rx = Arc::new(Mutex::new(load_request_rx));

		let (loaded_img_tx, loaded_img_rx) = channel();

		let mut join_handles = Vec::new();
		for _ in 0..threads {
			let running = running.clone();
			let request_recv = load_request_rx.clone();
			let request_send = load_request_tx.clone();
			let img_sender = loaded_img_tx.clone();
			join_handles.push(thread::spawn(move || {
				Self::thread_loop(running, request_recv, request_send, img_sender);
			}));
		}

		ImageLoader {
			running,
			join_handles: Some(join_handles),

			image_rx: loaded_img_rx,
			path_tx: load_request_tx,
		}
	}

	fn thread_loop(
		running: Arc<AtomicBool>,
		request_recv: Arc<Mutex<Receiver<LoadRequest>>>,
		request_send: Sender<LoadRequest>,
		img_sender: Sender<LoadResult>,
	) {
		// The size was an arbitrary choice made with the argument that this should be
		// enough to fit enough image file info to determine the format.

		//let mut DEBUG_FAIL_COUNT = 0;
		while running.load(Ordering::Acquire) {
			let request;
			{
				// It is very important that we release the mutex before starting to load the image
				let load_request = request_recv.lock().unwrap();
				let priority = PRIORITY_REQUEST_ID.load(Ordering::SeqCst);
				request = load_request.recv().unwrap();
				let focus_test_passed =
					priority == request.req_id || priority == NON_EXISTENT_REQUEST_ID;
				if !focus_test_passed {
					//println!("Priority test failed, priority was {}", priority);
					//DEBUG_FAIL_COUNT += 1;
					//if DEBUG_FAIL_COUNT > 4 { panic!("DEBUG_FAIL_COUNT > 4"); }
					// Just place the request neatly back to the request queue.
					request_send.send(request).unwrap();
					continue;
				}
			};
			Self::load_and_send(&img_sender, request);
		}
	}

	pub fn try_recv_prefetched(&mut self) -> std::result::Result<LoadResult, TryRecvError> {
		self.image_rx.try_recv()
	}

	pub fn send_load_request(&mut self, request: LoadRequest) {
		self.path_tx.send(request).unwrap();
	}

	fn load_and_send(img_sender: &Sender<LoadResult>, request: LoadRequest) {
		fn try_load_and_send(img_sender: &Sender<LoadResult>, request: &LoadRequest) -> Result<()> {
			let metadata = fs::metadata(&request.path)?;
			img_sender.send(LoadResult::Start { req_id: request.req_id, metadata }).unwrap();
			complex_load_image(&request.path, true, request.req_id, |frame| {
				img_sender.send(frame).unwrap();
				Ok(())
			})?;
			Ok(())
		}

		img_sender
			.send(match try_load_and_send(img_sender, &request) {
				Ok(()) => LoadResult::Done { req_id: request.req_id },
				Err(error) => {
					eprintln!(
						"Request #{}: Error occurred while loading file {:?}\n    {}",
						request.req_id, request.path, error,
					);
					LoadResult::Failed { req_id: request.req_id }
				}
			})
			.unwrap();
	}
}

impl Drop for ImageLoader {
	fn drop(&mut self) {
		self.running.store(false, Ordering::Release);
		if let Some(join_handles) = self.join_handles.take() {
			for _ in join_handles.iter() {
				self.path_tx.send(LoadRequest { req_id: 0, path: PathBuf::from("") }).unwrap();
			}

			for handle in join_handles.into_iter() {
				if let Err(err) = handle.join() {
					eprintln!("Error occurred while joining handle {:?}", err);
				}
			}
		}
	}
}
