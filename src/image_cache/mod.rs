use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

use gelatin::glium;

use glium::texture::SrgbTexture2d;

pub mod image_loader;
use self::image_loader::*;

mod pending_requests;
use pending_requests::PendingRequests;

pub mod errors {
	use crate::image_cache::image_loader;
	use gelatin::glium::texture;
	use gelatin::image;
	use std::io;

	error_chain! {
		foreign_links {
			Io(io::Error) #[doc = "Error during IO"];
			TextureCreationError(texture::TextureCreationError);
			ImageRsError(image::ImageError);
			TextureLoaderError(image_loader::errors::Error);
		}
		errors {
			WaitingOnLoader {
				description("ImageCache is waiting for loader to send result")
				display("ImageCache is waiting for loader to send result")
			}
			FailedToLoadImage(req_id: u32) {
				display("Failed to load #{}", req_id)
			}
		}
	}
}

pub use self::errors::Result;
use self::errors::*;

pub fn get_image_size_estimate(width: u32, height: u32) -> isize {
	// In an RGBA image, each pixel is 4 bytes.
	// counting all the mipmaps would add an additionnal multiplier of around ~1.6
	// but only the gpu textures have mip maps so just multiply by 1.5
	// 4 x 1.5 gives the factor 6.
	(width * height * 6) as isize
}

pub fn get_anim_size_estimate(frames: &[AnimationFrameTexture]) -> isize {
	frames
		.iter()
		.map(|frame| get_image_size_estimate(frame.texture.width(), frame.texture.height()))
		.sum()
}

struct ImageDescriptor {
	dir_entry: fs::DirEntry,

	/// Sometimes also abbreviated as `req_id` is used as a more efficient replacement
	/// of a PathBuf to identify a file load request.
	request_id: u32,
	// /// If an ongoing request
	// loaded: bool,
	//frame_count: Option<u32>, // it is evaluated in an on-demand fashion
}
impl ImageDescriptor {
	fn from_entry(dir_entry: fs::DirEntry, request_id: u32) -> ImageDescriptor {
		ImageDescriptor { dir_entry, request_id /* loaded: false */ }
	}
}

/// The request sender function must process all prefetched requests to avoid
/// hitting the pending request limit (MAX_PENDING_REQUESTS). The display is needed
/// for this processing but it would be incorrect to require the dispaly for prefetch
/// requests
enum RequestKind<'a> {
	NonPriority,
	Priority { display: &'a glium::Display },
}

impl<'a> RequestKind<'a> {
	pub fn priority(self) -> bool {
		match self {
			RequestKind::Priority { .. } => true,
			RequestKind::NonPriority => false,
		}
	}
}

#[derive(Clone)]
pub struct AnimationFrameTexture {
	pub texture: Rc<SrgbTexture2d>,
	pub delay_nano: u64,
	pub angle: f32,
}

struct CachedTexture {
	/// Contains the load request id
	_req_id: u32,
	needs_update: bool,
	mod_time: Option<SystemTime>,

	/// This is false if there are frames from the animation that haven't been added.
	/// This is used when requesting a frame that's outside of `frames`.
	/// In such a case the value of `fully_loaded` is inspected and if the image
	/// is loaded the frame number is wrapped around so that it falls onto the range.
	/// If it's not fully loaded yet a `WaitingOnLoader` error is returned.
	fully_loaded: bool,

	/// - `false` if loading is still in progress or if succeeded.
	/// - `true` if this failed to load,
	failed: bool,

	/// If the target file is an image this vector will have a single texture once the
	/// image uploaded to the GPU. If the target file is an animated image like a gif,
	/// these the frames
	frames: Vec<AnimationFrameTexture>,
}

/// The process of loading an image (or animation frame) consists of the following steps.
/// Note that even still images are handled as 1 frame long animations as there is
/// semantically no difference between those and this keeps the code relatively simple.
///
/// - ImageCache - Load request sent
///     - An entry representing the request is created in `ImageCache::ongoing_requests`
/// - Worker thread - Decodes an image from the byte stream into a CPU-side buffer. Send this back on a channel
///     - Decoded pixel data on channel (CPU)
/// - ImageCache - Prefetched image received from worker thread
///     - Decoded pixel data in `ImageCache::prefetched`
/// - ImageCache - Prefetched image is uploaded to a GPU texture from the CPU
///     - Pixel data is on the gpu referred to by `ImageCache::texture_cache`
///
pub struct ImageCache {
	dir_path: PathBuf,
	//current_name: OsString,
	current_file_idx: usize,
	current_frame_idx: usize,
	dir_files: Vec<ImageDescriptor>,

	remaining_capacity: isize,
	total_capacity: isize,
	curr_est_size: isize,

	/// A monotonically increasing integer used for identifying
	/// each load request
	current_req_id: u32,

	pending_requests: PendingRequests,
	texture_cache: BTreeMap<u32, CachedTexture>,
	loader: ImageLoader,
}

/// This is a store for the supported images loaded from a folder
///
/// The basic idea is to have a few images already in the memory while an image is shown on the screen
impl ImageCache {
	const MAX_PENDING_REQUESTS: usize = 5;

	/// # Arguments
	/// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
	pub fn new(capacity: isize, threads: u32) -> ImageCache {
		ImageCache {
			dir_path: PathBuf::new(),
			current_file_idx: 0,
			current_frame_idx: 0,
			dir_files: Vec::new(),

			remaining_capacity: capacity,
			total_capacity: capacity,
			curr_est_size: 1000, // 1 kb, an optimistic estimate for the image size before anything is loaded

			current_req_id: 0,
			pending_requests: PendingRequests::new(),
			texture_cache: BTreeMap::new(),
			loader: ImageLoader::new(threads),
		}
	}

	pub fn _cached_from_dir(&self) -> Vec<bool> {
		let mut result = Vec::with_capacity(self.dir_files.len());
		for desc in self.dir_files.iter() {
			result.push(self.texture_cache.contains_key(&desc.request_id));
		}
		result
	}

	pub fn current_filename(&self) -> OsString {
		match self.dir_files.get(self.current_file_idx) {
			Some(desc) => desc.dir_entry.file_name(),
			None => OsString::new(),
		}
	}

	pub fn current_file_path(&self) -> PathBuf {
		self.dir_path.join(self.current_filename())
	}

	pub fn current_file_index(&self) -> usize {
		self.current_file_idx
	}

	pub fn current_dir_len(&self) -> usize {
		self.dir_files.len()
	}

	/// Returns tru if and only if the current image has been fully loaded and it has a single frame.
	pub fn loaded_still_image(&self) -> bool {
		if let Some(desc) = self.dir_files.get(self.current_file_idx) {
			if let Some(img) = self.texture_cache.get(&desc.request_id) {
				if img.fully_loaded && img.frames.len() == 1 {
					return true;
				}
			}
		}
		false
	}

	/// Fetches the contents of the folder and stores the list of image filenames to know which
	/// files will be the next and previous.
	///
	/// Tries to locate the image that was the current image before calling the function and
	/// keeping it current. If that filename is not found, than it tries to preserve the previous
	/// file index instead of the filename. If there is no such an index in the folder, it resets
	/// the index to 0 making the current file the first one in the folder.
	///
	/// Returns the error that might occure while fetching the files from the directory. Otherwise
	/// returns `Ok(())`
	pub fn update_directory(&mut self) -> Result<()> {
		self.dir_files = self.collect_directory()?;
		let curr_filename = self.current_filename();

		// TODO add a `needs_update` field to the cached texture struct
		// and set all of those to true to indicate that the an update directory
		// call was made since those were created and they should all be
		// checked against the modification time of the file system file.
		for texture in self.texture_cache.values_mut() {
			texture.needs_update = true;
		}

		for (index, desc) in self.dir_files.iter().enumerate() {
			if desc.dir_entry.file_name() == curr_filename {
				self.current_file_idx = index;
				return Ok(());
			}
		}

		if self.dir_files.len() <= self.current_file_idx && !self.dir_files.is_empty() {
			self.current_file_idx = 0;
		}

		Ok(())
	}

	pub fn load_at_index(
		&mut self,
		display: &glium::Display,
		index: usize,
		frame_id: Option<isize>,
	) -> Result<(AnimationFrameTexture, PathBuf)> {
		let path = self
			.dir_files
			.get(index)
			.ok_or_else(|| {
				format!(
					"Index {} is out of bounds of the current directory {:?}",
					index, self.dir_path
				)
			})?
			.dir_entry
			.path();

		let result = self.load_specific(display, &path, frame_id)?;
		self.current_file_idx = index;
		Ok((result, path))
	}

	fn get_file_index(&self, parent: &Path, file_name: &OsStr) -> Result<usize> {
		for (index, desc) in self.dir_files.iter().enumerate() {
			if desc.dir_entry.file_name() == file_name {
				return Ok(index);
			}
		}
		let mut path = parent.to_owned();
		path.push(file_name);
		bail!("Image at path {:?} not found", path);
	}

	/// Returns `Err(errors::Error::from_kind(errors::ErrorKind::WaitingOnLoader))`
	/// when the image
	pub fn load_specific(
		&mut self,
		display: &glium::Display,
		path: &Path,
		frame_id: Option<isize>,
	) -> Result<AnimationFrameTexture> {
		self.receive_prefetched();
		let target_file_name;
		let parent;
		if path.is_dir() {
			parent = path.to_owned();
			target_file_name = None;
		} else {
			let filename_and_parent = get_file_name_and_parent(path)?;
			target_file_name = Some(filename_and_parent.0);
			parent = filename_and_parent.1;
		}

		let new_file_index;
		if let Some(target_file_name) = target_file_name {
			self.change_directory_with_filename(&parent, &target_file_name)?;
			new_file_index = self.get_file_index(&parent, &target_file_name)?;
		} else {
			self.change_directory(&parent)?;
			self.current_file_idx = 0;
			self.current_frame_idx = 0;
			new_file_index = 0;
		}
		if self.dir_path != parent {
			self.send_request_for_index(self.current_file_idx, RequestKind::Priority { display });
			return Err(errors::Error::from_kind(errors::ErrorKind::WaitingOnLoader));
		}
		let requested_frame_id = match frame_id {
			Some(frame_id) => frame_id,
			None if self.current_file_idx != new_file_index => 0,
			None => self.current_frame_idx as isize,
		};
		self.current_file_idx = new_file_index;
		self.refresh_cache();
		self.try_getting_requested_image(display, self.current_file_idx, requested_frame_id)
	}

	fn refresh_cache(&mut self) {
		let cache = mem::take(&mut self.texture_cache);

		// Delete all entries that are outside the range of files around the current file
		// allowed by the capacity.
		// Walk through our list of directory entries sorted by their distance from the current
		// file and in each step remove an entry from the cache until we reach the desired cache
		// size
		let mut sorted_files: Vec<_> = cache.into_iter().enumerate().collect();
		sorted_files.sort_unstable_by_key(|&(index, _)| {
			(index as isize - self.current_file_idx as isize).abs()
		});
		self.remaining_capacity = self.total_capacity;
		sorted_files.retain(|(_, (_, texture))| {
			// TODO consider retaining individual frames.
			let all_frames_size = get_anim_size_estimate(&texture.frames);

			if self.remaining_capacity > (all_frames_size + self.curr_est_size) {
				self.remaining_capacity -= all_frames_size;
				true
			} else {
				false
			}
		});

		self.texture_cache = sorted_files.into_iter().map(|(_, entry)| entry).collect();
	}

	pub fn load_next(
		&mut self,
		display: &glium::Display,
	) -> Result<(AnimationFrameTexture, PathBuf)> {
		self.load_jump(display, 1, 0)
	}
	pub fn load_prev(
		&mut self,
		display: &glium::Display,
	) -> Result<(AnimationFrameTexture, PathBuf)> {
		self.load_jump(display, -1, 0)
	}

	pub fn load_jump(
		&mut self,
		display: &glium::Display,
		file_jump_count: i32,
		frame_jump_count: isize,
	) -> Result<(AnimationFrameTexture, PathBuf)> {
		if file_jump_count == 0 {
			let _path = self.current_file_path();
			// Here, it is possible that the current image was already
			// requested but not yet loaded.
			let target_frame = self.current_frame_idx as isize + frame_jump_count;
			let requested =
				self.try_getting_requested_image(display, self.current_file_idx, target_frame);
			return requested.map(|t| (t, self.current_file_path()));
		} else {
			self.current_frame_idx = 0;
		}

		if self.dir_files.is_empty() {
			bail!("Folder is empty or no folder was open when trying to load image.");
		}

		// rem_euclid calculates the least nonnegative remainder
		let target_index = (self.current_file_idx as isize + file_jump_count as isize)
			.rem_euclid(self.dir_files.len() as isize) as usize;

		let target_path = self.dir_files[target_index].dir_entry.path();
		let result = self.load_specific(display, &target_path, None)?;
		self.current_file_idx = target_index;

		Ok((result, target_path))
	}

	fn receive_prefetched(&mut self) {
		use std::sync::mpsc::TryRecvError;
		loop {
			match self.loader.try_recv_prefetched() {
				Ok(load_result) => {
					self.pending_requests.add_load_result(load_result);
				}
				Err(TryRecvError::Disconnected) => panic!("Channel disconnected unexpectidly."),
				Err(TryRecvError::Empty) => break,
			}
		}
	}

	pub fn process_prefetched(&mut self, display: &glium::Display) -> Result<()> {
		self.receive_prefetched();
		let mut uploaded_one = false;
		let req_ids = self.pending_requests.get_all_ids();
		let mut retval = Ok(());
		for id in req_ids {
			if let Some(results) = self.pending_requests.take_results(id) {
				for result in results {
					match self.upload_to_texture(display, result) {
						Ok(_) => uploaded_one = true,
						// it's okay to ignore if the image falied to load here, this is just pre-fetch.
						Err(Error(ErrorKind::FailedToLoadImage(..), ..)) => {}
						Err(e) => {
							retval = Err(e);
							break;
						}
					}
				}
			}

			if retval.is_err() {
				return retval;
			}
			if uploaded_one {
				break;
			}
		}
		Ok(())
	}

	/// Negative frame numbers are allowed. So are larger-than-total-frame-count frame numbers.
	///
	/// This is because this function call will often happen when the animation is not fully loaded yet.
	/// So in order to minimize complexity of the functions calling this one, frame ids that are out of
	/// bounds are allowed and will be wraped around if needed within this function.
	fn try_getting_requested_image(
		&mut self,
		display: &glium::Display,
		file_index: usize,
		frame_id: isize,
	) -> Result<AnimationFrameTexture> {
		let path;
		let req_id;
		if let Some(desc) = self.dir_files.get(file_index) {
			path = desc.dir_entry.path();
			req_id = desc.request_id;
		} else {
			bail!("Index {} was not in the folder.", file_index);
		}

		// Check if it's among the prefetched, and upload it, if it is
		if let Some(results) = self.pending_requests.take_results(req_id) {
			for load_result in results {
				self.upload_to_texture(display, load_result)?;
			}
			// And just let the next blok deal with locating the appropriate frame.
		}

		// Check if it is inside the texture cache first
		if let Some(tex) = self.texture_cache.get(&req_id) {
			if tex.failed {
				return Err(Error::from_kind(ErrorKind::FailedToLoadImage(req_id)));
			}
			let modified = fs::metadata(&path).ok().and_then(|m| m.modified().ok());
			let mut get_from_cache = false;
			if let Some(curr_mod_time) = modified {
				if let Some(mod_time) = tex.mod_time {
					if mod_time == curr_mod_time {
						get_from_cache = true;
					}
				}
			} else {
				get_from_cache = true;
			}
			if get_from_cache {
				let count = tex.frames.len() as isize;
				if tex.fully_loaded || (frame_id >= 0 && frame_id < count) {
					let wrapped_id;
					if frame_id < 0 {
						wrapped_id = count + (frame_id % count);
					} else {
						wrapped_id = frame_id % count;
					}
					if let Some(frame) = tex.frames.get(wrapped_id as usize) {
						self.current_frame_idx = wrapped_id as usize;
						return Ok(frame.clone());
					}
				}
			}
			return Err(Error::from_kind(ErrorKind::WaitingOnLoader));
		}
		if self.pending_requests.contains(&req_id) {
			PRIORITY_REQUEST_ID.store(req_id, Ordering::SeqCst);
			return Err(Error::from_kind(ErrorKind::WaitingOnLoader));
		}
		self.send_request_for_index(self.current_file_idx, RequestKind::Priority { display });
		// If the texture is not in the cache just throw our hands in the air
		// and tell the caller that we gotta wait for the loader to load this texture.
		Err(Error::from_kind(ErrorKind::WaitingOnLoader))
	}

	fn upload_to_texture(
		&mut self,
		display: &glium::Display,
		load_result: LoadResult,
	) -> Result<Option<AnimationFrameTexture>> {
		use std::collections::btree_map::Entry;
		match load_result {
			LoadResult::Start { req_id, metadata } => {
				let curr_mod_time = metadata.modified().ok();
				if let Some(cancelled) = self.pending_requests.cancelled(&req_id) {
					if cancelled {
						return Ok(None);
					}
				} else {
					return Ok(None);
				}
				match self.texture_cache.entry(req_id) {
					Entry::Vacant(entry) => {
						entry.insert(CachedTexture {
							_req_id: req_id,
							needs_update: false,
							fully_loaded: false,
							mod_time: curr_mod_time,
							failed: false,
							frames: Vec::new(),
						});
					}
					Entry::Occupied(mut entry) => {
						let mut overwrite = true;
						if let Some(curr_mod_time) = curr_mod_time {
							let cached = entry.get();
							if let Some(existing_mod_time) = cached.mod_time {
								if existing_mod_time == curr_mod_time {
									overwrite = false;
								}
							}
						}
						if overwrite {
							let old_size_estimate = get_anim_size_estimate(&entry.get().frames);
							self.remaining_capacity += old_size_estimate;
							let mut_entry = entry.get_mut();
							mut_entry.frames.clear();
							mut_entry.mod_time = curr_mod_time;
						}
					}
				}
				Ok(None)
			}
			LoadResult::Frame { req_id, image, delay_nano, orientation } => {
				if let Some(cancelled) = self.pending_requests.cancelled(&req_id) {
					if cancelled {
						return Ok(None);
					}
				} else {
					return Ok(None);
				}
				let size_estimate = get_image_size_estimate(image.width(), image.height());
				if let Some(entry) = self.texture_cache.get_mut(&req_id) {
					let texture = Rc::new(texture_from_image(display, image)?);
					let anim_frame =
						AnimationFrameTexture { texture, delay_nano, angle: orientation.to_rad() };
					entry.frames.push(anim_frame.clone());
					self.remaining_capacity -= size_estimate;
					return Ok(Some(anim_frame));
				}
				Ok(None)
			}
			LoadResult::Done { req_id } => {
				if let Some(tex) = self.texture_cache.get_mut(&req_id) {
					tex.fully_loaded = true;
				}
				PRIORITY_REQUEST_ID.compare_and_swap(
					req_id,
					NON_EXISTENT_REQUEST_ID,
					Ordering::SeqCst,
				);
				self.pending_requests.set_finished(&req_id);
				Ok(None)
			}
			LoadResult::Failed { req_id } => {
				if let Some(tex) = self.texture_cache.get_mut(&req_id) {
					tex.fully_loaded = true;
					tex.failed = true;
				}
				PRIORITY_REQUEST_ID.compare_and_swap(
					req_id,
					NON_EXISTENT_REQUEST_ID,
					Ordering::SeqCst,
				);
				self.pending_requests.set_finished(&req_id);
				Err(errors::Error::from_kind(errors::ErrorKind::FailedToLoadImage(req_id)))
			}
		}
	}

	pub fn prefetch_neighbors(&mut self) {
		let mut index = self.current_file_idx;

		// Send as many load requests so that the estimated total will just fill the cache
		let mut estimated_remaining_cap = self.remaining_capacity;

		while estimated_remaining_cap > self.curr_est_size {
			// Send a load request for the closest file not in the cache or outdated
			index += 1;
			if self.prefetch_at_index(index) {
				estimated_remaining_cap -= self.curr_est_size;
			} else {
				break;
			}
		}
	}

	pub fn prefetch_at_index(&mut self, index: usize) -> bool {
		if self.remaining_capacity > self.curr_est_size {
			return self.send_request_for_index(index, RequestKind::NonPriority);
		}
		false
	}

	/// This is almost identical to `prefetch_at_index` but this function
	/// does not check the `remaining_capacity`.
	fn send_request_for_index(&mut self, index: usize, kind: RequestKind) -> bool {
		if let RequestKind::Priority { display } = kind {
			if self.pending_requests.len() >= Self::MAX_PENDING_REQUESTS {
				if let Err(e) = self.process_prefetched(display) {
					eprintln!("Error while processing prefetched images:\n{}", e);
				}
			}
		}
		if self.pending_requests.len() >= Self::MAX_PENDING_REQUESTS {
			return false;
		}
		if let Some(desc) = self.dir_files.get(index) {
			let file = &desc.dir_entry;
			let file_path = file.path();
			let mut cache_enty_invalid = false;
			if let Some(texture) = self.texture_cache.get_mut(&desc.request_id) {
				if !texture.needs_update {
					return false;
				} else {
					texture.needs_update = false;
					if let Some(existing_mod_time) = texture.mod_time {
						let new_mod_time =
							fs::metadata(&file_path).ok().and_then(|m| m.modified().ok());
						if let Some(new_mod_time) = new_mod_time {
							if new_mod_time == existing_mod_time {
								return false;
							} else {
								cache_enty_invalid = true;
							}
						}
					}
				}
			}
			if cache_enty_invalid {
				self.texture_cache.remove(&desc.request_id);
			}
			if kind.priority() {
				PRIORITY_REQUEST_ID.store(desc.request_id, Ordering::SeqCst);
			}
			if self.pending_requests.contains(&desc.request_id) {
				return false;
			}
			let req_id = desc.request_id;
			let request = LoadRequest { req_id, path: file_path };
			self.pending_requests.add_request(request.clone());
			self.loader.send_load_request(request);
			return true;
		}
		false
	}

	fn change_directory(&mut self, dir_path: &Path) -> Result<()> {
		self.texture_cache.clear();
		self.remaining_capacity = self.total_capacity;

		// Cancel all pending load requests
		for (_, request) in self.pending_requests.iter_mut() {
			request.cancel();
		}

		self.dir_path = dir_path.to_owned();
		self.dir_files = self.collect_directory()?;
		Ok(())
	}

	fn change_directory_with_filename(&mut self, dir_path: &Path, filename: &OsStr) -> Result<()> {
		self.change_directory(dir_path)?;

		// Look up the index of the filename in the directory
		for (index, desc) in self.dir_files.iter().enumerate() {
			if desc.dir_entry.file_name() == filename {
				self.current_file_idx = index;
				return Ok(());
			}
		}

		bail!("Could not find file {:?} in directory {:?}", filename, dir_path);
	}

	fn collect_directory(&mut self) -> Result<Vec<ImageDescriptor>> {
		let start = std::time::Instant::now();
		let mut dir_files: Vec<_> = fs::read_dir(&self.dir_path)?
			.filter_map(|x| match x {
				Ok(entry) => match entry.file_type() {
					Ok(file_type) => {
						if file_type.is_file() || file_type.is_symlink() {
							if is_file_supported(entry.path().as_path()) {
								self.current_req_id += 1;
								Some(ImageDescriptor::from_entry(entry, self.current_req_id))
							} else {
								None
							}
						} else {
							None
						}
					}
					Err(_) => None,
				},
				Err(_) => None,
			})
			.collect();

		let total_time = start.elapsed();
		println!("Collected directory in: {} ms", total_time.as_millis());
		dir_files.sort_unstable_by(|a, b| {
			lexical_sort::natural_lexical_cmp(
				&a.dir_entry.file_name().to_string_lossy(),
				&b.dir_entry.file_name().to_string_lossy(),
			)
		});

		Ok(dir_files)
	}
}

fn get_file_name_and_parent(path: &Path) -> Result<(OsString, PathBuf)> {
	let file_name = match path.file_name() {
		Some(f) => f.to_owned(),
		None => bail!("Could not get file name from path {:?}", path),
	};
	let parent = match path.parent() {
		Some(p) => {
			if p == Path::new("") {
				Path::new(".").canonicalize()?
			} else {
				p.canonicalize()?
			}
		}
		None => {
			let mut path = path.canonicalize()?;
			if !path.pop() {
				bail!("Could not get parent directory of {:?}", path);
			}
			path
		}
	};

	Ok((file_name, parent))
}
