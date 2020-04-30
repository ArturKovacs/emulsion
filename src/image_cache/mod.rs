use std::collections::{BTreeMap, HashMap};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

use gelatin::glium;

use glium::texture::SrgbTexture2d;

mod image_loader;
use self::image_loader::*;

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
			FailedToLoadImage {}
		}
	}
}

pub use self::errors::Result;
use self::errors::*;

pub fn get_image_size_estimate(dimensions: (u32, u32)) -> u32 {
	// counting all the mipmaps would add an additionnal multiplier of around ~1.6
	// but only the gpu textures have mip maps so just multiply by 1.5
	((dimensions.0 * dimensions.1 * 4) as f32 * 1.5) as u32
}

pub fn get_anim_size_estimate(frames: &[AnimationFrameTexture]) -> u32 {
	let mut size = 0;
	for frame in frames {
		let dimensions = frame.texture.dimensions();
		size += get_image_size_estimate(dimensions);
	}
	size
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

/// This struct is used in a map to determine the appropriate file when
/// a load result comes in. (Load results identify the file with the request id only)
/// See: `ImageCache::ongoing_requests`
struct OngoingRequest {
	path: PathBuf,
	cancelled: bool,
	// mod_time: Option<SystemTime>,

	// /// The index of this file within the sorted folder.
	// /// indexing `ImageCache::dir_files` with this will return the corresponding
	// /// file.
	// index: usize,
}

#[derive(Clone)]
pub struct AnimationFrameTexture {
	pub texture: Rc<SrgbTexture2d>,
	pub delay_nano: u64,
}

struct CachedTexture {
	/// Contains the load request id
	req_id: u32,
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
/// 	- An entry representing the request is created in `ImageCache::ongoing_requests`
/// - Worker thread - Decodes an image from the byte stream into a CPU-side buffer. Send this back on a channel
/// 	- Decoded pixel data on channel (CPU)
/// - ImageCache - Prefetched image received from worker thread
/// 	- Decoded pixel data in `ImageCache::prefetched`
/// - ImageCache - Prefetched image is uploaded to a GPU texture from the CPU
/// 	- Pixel data is on the gpu referred to by `ImageCache::texture_cache`
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
	requested_images: i32,

	/// A monotonically increasing integer used for identifying
	/// each load request
	current_req_id: u32,

	/// Keeps track of all requests that have been sent
	/// but for which no response has been received
	ongoing_requests: HashMap<u32, OngoingRequest>,
	prefetched: HashMap<PathBuf, Vec<LoadResult>>,
	texture_cache: BTreeMap<PathBuf, CachedTexture>,
	loader: ImageLoader,
}

/// This is a store for the supported images loaded from a folder
///
/// The basic idea is to have a few images already in the memory while an image is shown on the screen
impl ImageCache {
	const MAX_PENDING_PREFETCH_REQUESTS: i32 = 5;

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
			requested_images: 0,

			current_req_id: 0,
			ongoing_requests: HashMap::new(),
			prefetched: HashMap::new(),
			texture_cache: BTreeMap::new(),
			loader: ImageLoader::new(threads),
		}
	}

	pub fn cached_from_dir(&self) -> Vec<bool> {
		let mut result = Vec::with_capacity(self.dir_files.len());
		for i in 0..self.dir_files.len() {
			let file_path = self.dir_files[i].dir_entry.path();
			result.push(self.texture_cache.contains_key(&file_path));
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

		if self.dir_files.len() > self.current_file_idx {
			return Ok(());
		} else if !self.dir_files.is_empty() {
			self.current_file_idx = 0;
			return Ok(());
		}

		Ok(())
	}

	pub fn load_at_index(
		&mut self,
		display: &glium::Display,
		index: usize,
		frame_id: Option<isize>,
	) -> Result<(AnimationFrameTexture, OsString)> {
		let path = self
			.dir_files
			.get(index)
			.ok_or_else(|| {
				format!(
					"Index {} is out of bounds of the current directory '{}'",
					index,
					self.dir_path.to_str().unwrap()
				)
			})?
			.dir_entry
			.path();

		let result = self.load_specific(display, &path, frame_id)?;
		self.current_file_idx = index;
		Ok((result, path.file_name().unwrap_or_else(|| OsStr::new("")).to_owned()))
	}

	/// Returns `Err(errors::Error::from_kind(errors::ErrorKind::WaitingOnLoader))`
	/// when the image
	pub fn load_specific(
		&mut self,
		display: &glium::Display,
		path: &Path,
		frame_id: Option<isize>,
	) -> Result<AnimationFrameTexture> {
		let path = path.canonicalize()?;

		let target_file_name = match path.file_name() {
			Some(filename) => filename.to_owned(),
			None => bail!(format!("Could not get filename from path '{}'", path.to_str().unwrap())),
		};

		let parent = path.parent().ok_or("Could not get parent directory")?.to_owned();

		// Lets just process incoming images
		//self.process_prefetched(display)?;
		let requested_frame_id;
		self.receive_prefetched();
		if self.dir_path != parent {
			self.texture_cache.clear();
			self.prefetched.clear();
			self.remaining_capacity = self.total_capacity;
			self.change_directory(&parent, &target_file_name)?;
			self.send_request_for_index(self.current_file_idx);
			return Err(errors::Error::from_kind(errors::ErrorKind::WaitingOnLoader));
		} else {
			let mut image_found = false;
			let mut new_file_index = self.current_file_idx;
			for (index, desc) in self.dir_files.iter().enumerate() {
				if desc.dir_entry.file_name() == target_file_name {
					image_found = true;
					new_file_index = index;
					break;
				}
			}
			if !image_found {
				// The image must have been removed from the folder. It cannot be loaded
				bail!("Image not found at '{:?}'", path);
			}
			if let Some(frame_id) = frame_id {
				requested_frame_id = frame_id;
			} else if self.current_file_idx != new_file_index {
				requested_frame_id = 0;
			} else {
				requested_frame_id = self.current_frame_idx as isize;
			}
			self.current_file_idx = new_file_index;
			let mut tmp_cache = BTreeMap::new();
			mem::swap(&mut self.texture_cache, &mut tmp_cache);

			// Delete all entries that are outside the range of files around the current file
			// allowed by the capacity.
			// Walk through our list of directory entries sorted by their distance from the current
			// file and in each step remove an entry from the cache until we reach the desired cache
			// size
			let mut sorted_files: Vec<_> = tmp_cache.into_iter().enumerate().collect();
			sorted_files.sort_unstable_by_key(|(index, _)| {
				(*index as isize - self.current_file_idx as isize).abs()
			});
			let mut remaining_capacity = self.total_capacity;
			sorted_files.retain(|cached| {
				let (_path, texture) = &cached.1;
				let mut all_frames_size = 0;
				// TODO consider retaining individual frames.
				for t in texture.frames.iter() {
					// Thew new file has to fit in the cache after this operation
					// which is why we multiply the estimated size by two
					let dimensions = (t.texture.width(), t.texture.height());
					all_frames_size += get_image_size_estimate(dimensions) as isize;
				}
				let shoudl_retain = remaining_capacity > all_frames_size + self.curr_est_size;
				if shoudl_retain {
					remaining_capacity -= all_frames_size;
				}
				shoudl_retain
			});
			tmp_cache = sorted_files.into_iter().map(|(_, entry)| entry).collect();
			mem::swap(&mut self.texture_cache, &mut tmp_cache);
			self.remaining_capacity = remaining_capacity;
		}

		self.try_getting_requested_image(display, &path, requested_frame_id)
	}

	pub fn load_next(
		&mut self,
		display: &glium::Display,
	) -> Result<(AnimationFrameTexture, OsString)> {
		self.load_jump(display, 1, 0)
	}
	pub fn load_prev(
		&mut self,
		display: &glium::Display,
	) -> Result<(AnimationFrameTexture, OsString)> {
		self.load_jump(display, -1, 0)
	}

	pub fn load_jump(
		&mut self,
		display: &glium::Display,
		file_jump_count: i32,
		frame_jump_count: isize,
	) -> Result<(AnimationFrameTexture, OsString)> {
		if file_jump_count == 0 {
			let path = self.current_file_path();
			// Here, it is possible that the current image was already
			// requested but not yet loaded.
			let target_frame = self.current_frame_idx as isize + frame_jump_count;
			let requested = self.try_getting_requested_image(display, &path, target_frame);
			return requested.map(|t| (t, self.current_filename()));
		} else {
			self.current_frame_idx = 0;
		}

		if self.dir_files.is_empty() {
			return Err("Folder is empty or no folder was open when trying to load image.".into());
		}

		let mut target_index = (self.current_file_idx as isize + file_jump_count as isize)
			% self.dir_files.len() as isize;
		if target_index < 0 {
			target_index += self.dir_files.len() as isize;
		}

		let target_path = self.dir_files[target_index as usize].dir_entry.path();
		let result = self.load_specific(display, &target_path, None)?;
		self.current_file_idx = target_index as usize;

		Ok((result, target_path.file_name().unwrap_or_else(|| OsStr::new("")).to_owned()))
	}

	fn receive_prefetched(&mut self) {
		use std::collections::hash_map::Entry;
		use std::sync::mpsc::TryRecvError;
		loop {
			match self.loader.try_recv_prefetched() {
				Ok(load_result) => {
					match load_result {
						LoadResult::Failed { .. } | LoadResult::Done { .. } => {
							self.requested_images -= 1;
						}
						_ => {}
					}
					let request;
					if let Some(req) = self.ongoing_requests.get(&load_result.req_id()) {
						request = req;
					} else {
						continue;
					}
					match self.prefetched.entry(request.path.clone()) {
						Entry::Vacant(entry) => {
							let mut result_vec = Vec::with_capacity(3);
							result_vec.push(load_result);
							entry.insert(result_vec);
						}
						Entry::Occupied(mut entry) => {
							entry.get_mut().push(load_result);
						}
					}
				}
				Err(TryRecvError::Disconnected) => panic!("Channel disconnected unexpectidly."),
				Err(TryRecvError::Empty) => break,
			}
		}
	}

	pub fn process_prefetched(&mut self, display: &glium::Display) -> Result<()> {
		self.receive_prefetched();
		// Prefetched are NOT ordered but for now this is better than nothing.
		let first_key = self.prefetched.iter().nth(0).map(|v| v.0.clone());
		if let Some(first_key) = first_key {
			if let Some(result_vec) = self.prefetched.remove(&first_key) {
				for result in result_vec {
					match self.upload_to_texture(display, result) {
						// it's okay to ignore if the image falied to load here, this is just pre-fetch.
						Err(Error (ErrorKind::FailedToLoadImage, ..)) => {} 
						Err(e) => return Err(e),
						_ => {}
					}
				}
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
		path: &Path,
		frame_id: isize,
	) -> Result<AnimationFrameTexture> {
		// Check if it's among the prefetched, and upload it, if it is
		if let Some(result_vec) = self.prefetched.remove(path) {
			for load_result in result_vec.into_iter() {
				self.upload_to_texture(display, load_result)?;
			}
			// And just let the next blok deal with locating the appropriate frame.
		}

		// Check if it is inside the texture cache first
		if let Some(tex) = self.texture_cache.get(path) {
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
					if tex.failed {
						return Err(Error::from_kind(ErrorKind::FailedToLoadImage));
					}
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
		// Check if it's been requested
		if let Some(img_desc) = self.dir_files.get(self.current_file_idx) {
			let req_id = img_desc.request_id;
			if self.ongoing_requests.contains_key(&req_id) {
				return Err(Error::from_kind(ErrorKind::WaitingOnLoader));
			}
		} else {
			// If it's not among the dir files then maybe there's no directory open
			// or some other error occured
			bail!("Not found in directory.");
		}
		self.send_request_for_index(self.current_file_idx);
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
				let request;
				if let Some(req) = self.ongoing_requests.get(&req_id) {
					request = req;
				} else {
					return Ok(None);
				}
				if request.cancelled {
					return Ok(None);
				}
				//request.mod_time = curr_mod_time;
				match self.texture_cache.entry(request.path.clone()) {
					Entry::Vacant(entry) => {
						entry.insert(CachedTexture {
							req_id,
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
							let old_size_estimate =
								get_anim_size_estimate(&entry.get().frames) as isize;
							self.remaining_capacity += old_size_estimate;
							let mut_entry = entry.get_mut();
							mut_entry.frames.clear();
							mut_entry.mod_time = curr_mod_time;
						}
					}
				}
				return Ok(None);
			}
			LoadResult::Frame { req_id, image, delay_nano } => {
				let request;
				if let Some(req) = self.ongoing_requests.get(&req_id) {
					request = req;
				} else {
					return Ok(None);
				}
				if request.cancelled {
					return Ok(None);
				}
				let size_estimate =
					get_image_size_estimate((image.width(), image.height())) as isize;
				if let Some(entry) = self.texture_cache.get_mut(&request.path) {
					let texture = Rc::new(texture_from_image(display, image)?);
					let anim_frame = AnimationFrameTexture { texture, delay_nano };
					entry.frames.push(anim_frame.clone());
					self.remaining_capacity -= size_estimate;
					return Ok(Some(anim_frame));
				}
				return Ok(None);
			}
			LoadResult::Done { req_id } => {
				let request;
				if let Some(req) = self.ongoing_requests.get(&req_id) {
					request = req;
				} else {
					return Ok(None);
				}
				if let Some(tex) = self.texture_cache.get_mut(&request.path) {
					tex.fully_loaded = true;
				}
				self.ongoing_requests.remove(&req_id);
				return Ok(None);
			}
			LoadResult::Failed { req_id } => {
				let request;
				if let Some(req) = self.ongoing_requests.get(&req_id) {
					request = req;
				} else {
					// If the request was cancelled, then it's OK that the image failed to load.
					// We don't want to confuse the higher level components with telling them
					// that something they don't even care about failed to load.
					return Ok(None);
				}
				if let Some(tex) = self.texture_cache.get_mut(&request.path) {
					tex.fully_loaded = true;
					tex.failed = true;
				}
				self.ongoing_requests.remove(&req_id);
				return Err(errors::Error::from_kind(errors::ErrorKind::FailedToLoadImage));
			}
		}
	}

	pub fn prefetch_neighbors(&mut self) {
		let mut index = self.current_file_idx;

		// Send as many load requests so that the estimated total will just fill the cache
		let mut estimated_remaining_cap = self.remaining_capacity;

		while estimated_remaining_cap > self.curr_est_size as isize {
			if self.requested_images >= Self::MAX_PENDING_PREFETCH_REQUESTS {
				break;
			}
			// Send a load request for the closest file not in the cache or outdated
			index += 1;
			if self.prefetch_at_index(index) {
				estimated_remaining_cap -= self.curr_est_size as isize;
			} else {
				break;
			}
		}
	}

	pub fn prefetch_at_index(&mut self, index: usize) -> bool {
		if self.requested_images >= Self::MAX_PENDING_PREFETCH_REQUESTS {
			return false;
		}
		if self.remaining_capacity > self.curr_est_size as isize {
			return self.send_request_for_index(index);
		}
		false
	}

	/// This is used for initiating a load when said load was directly
	/// requested by a higher level component through one of the public
	/// `load_...` fuctions.
	///
	/// This is almost identical to `prefetch_at_index` but different in that
	/// it does not care whether the new image will fit into the allowed
	/// remaining capacity.
	fn send_request_for_index(&mut self, index: usize) -> bool {
		if let Some(desc) = self.dir_files.get(index) {
			let file = &desc.dir_entry;
			let file_path = file.path();
			if self.ongoing_requests.contains_key(&desc.request_id) {
				return false;
			}
			let mut cache_enty_invalid = false;
			if let Some(texture) = self.texture_cache.get_mut(&file_path) {
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
				self.texture_cache.remove(&file_path);
			}
			let req_id = desc.request_id;
			self.ongoing_requests
				.insert(req_id, OngoingRequest { path: file_path.clone(), cancelled: false });
			self.loader.send_load_request(LoadRequest { req_id, path: file_path });
			self.requested_images += 1;
			return true;
		}
		false
	}

	fn change_directory(&mut self, dir_path: &Path, filename: &OsStr) -> Result<()> {
		// Cancel all pending load requests
		for (_, request) in self.ongoing_requests.iter_mut() {
			request.cancelled = true;
		}

		self.dir_path = dir_path.to_owned();
		self.dir_files = self.collect_directory()?;

		// Look up the index of the filename in the directory
		for (index, desc) in self.dir_files.iter().enumerate() {
			if desc.dir_entry.file_name() == filename {
				self.current_file_idx = index;
				return Ok(());
			}
		}

		Err(format!(
			"Could not find file '{}' in directory '{}'",
			filename.to_str().unwrap(),
			dir_path.to_str().unwrap()
		)
		.into())
	}

	fn collect_directory(&mut self) -> Result<Vec<ImageDescriptor>> {
		let mut dir_files: Vec<_> = fs::read_dir(&self.dir_path)?
			.filter_map(|x| match x {
				Ok(entry) => match entry.file_type() {
					Ok(file_type) => {
						if file_type.is_file() {
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

		dir_files.sort_unstable_by(|a, b| {
			alphanumeric_sort::compare_os_str(&a.dir_entry.file_name(), &b.dir_entry.file_name())
		});

		Ok(dir_files)
	}
}
