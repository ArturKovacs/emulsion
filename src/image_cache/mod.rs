use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;

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
			ImageLoadError(image::ImageError);
			TextureLoaderError(image_loader::errors::Error);
		}
	}
}

pub use self::errors::Result;
use self::errors::*;

struct ImageDescriptor {
	dir_entry: fs::DirEntry,
	//frame_count: Option<u32>, // it is evaluated in an on-demand fashion
}

impl ImageDescriptor {
	fn from_entry(dir_entry: fs::DirEntry) -> ImageDescriptor {
		ImageDescriptor { dir_entry /* frame_count: None */ }
	}
}

pub struct ImageCache {
	dir_path: PathBuf,
	//current_name: OsString,
	current_index: usize,
	dir_files: Vec<ImageDescriptor>,

	remaining_capacity: isize,
	total_capacity: isize,
	curr_est_size: isize,
	requested_images: i32,
	texture_cache: BTreeMap<OsString, CachedTexture>,

	loader: ImageLoader,
}

/// This is a store for the supported images loaded from a folder
///
/// The basic idea is to have a few images already in the memory while an image is shown on the screen
impl ImageCache {
	const MAX_PENDING_PREFETCH_REQUESTS: i32 = 4;

	/// # Arguments
	/// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
	pub fn new(capacity: isize, threads: u32) -> ImageCache {
		ImageCache {
			dir_path: PathBuf::new(),
			current_index: 0,
			dir_files: Vec::new(),

			remaining_capacity: capacity,
			total_capacity: capacity,
			curr_est_size: capacity,
			requested_images: 0,
			texture_cache: BTreeMap::new(),

			loader: ImageLoader::new(threads),
		}
	}

	pub fn cached_from_dir(&self) -> Vec<bool> {
		let mut result = Vec::with_capacity(self.dir_files.len());

		for i in 0..self.dir_files.len() {
			let file_name = self.dir_files[i].dir_entry.file_name();
			result.push(self.texture_cache.contains_key(&file_name));
		}

		result
	}

	pub fn current_filename(&self) -> OsString {
		match self.dir_files.get(self.current_index) {
			Some(desc) => desc.dir_entry.file_name(),
			None => OsString::new(),
		}
	}

	pub fn current_file_path(&self) -> PathBuf {
		self.dir_path.join(self.current_filename())
	}

	pub fn current_file_index(&self) -> usize {
		self.current_index
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
		let curr_filename = self.current_filename();
		self.dir_files = Self::collect_directory(self.dir_path.as_path())?;

		for (index, desc) in self.dir_files.iter().enumerate() {
			if desc.dir_entry.file_name() == curr_filename {
				self.current_index = index;
				return Ok(());
			}
		}

		if self.dir_files.len() > self.current_index {
			return Ok(());
		} else if !self.dir_files.is_empty() {
			self.current_index = 0;
			return Ok(());
		}

		Ok(())
	}

	pub fn load_at_index(
		&mut self,
		display: &glium::Display,
		index: usize,
	) -> Result<(Rc<SrgbTexture2d>, OsString)> {
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

		let result = self.load_specific(display, &path)?;

		self.current_index = index;

		Ok((result, path.file_name().unwrap_or_else(|| OsStr::new("")).to_owned()))
	}

	pub fn load_specific(
		&mut self,
		display: &glium::Display,
		path: &Path,
	) -> Result<Rc<SrgbTexture2d>> {
		use std::collections::btree_map::Entry;

		let path = path.canonicalize()?;

		let target_file_name = match path.file_name() {
			Some(filename) => filename.to_owned(),
			None => bail!(format!("Could not get filename from path '{}'", path.to_str().unwrap())),
		};

		let parent = path.parent().ok_or("Could not get parent directory")?.to_owned();

		// Lets just process incoming images
		self.process_prefetched(display)?;

		if self.dir_path != parent {
			self.texture_cache.clear();
			self.remaining_capacity = self.total_capacity;
			self.change_directory(parent, target_file_name.clone())?;
		} else {
			for (index, desc) in self.dir_files.iter().enumerate() {
				if desc.dir_entry.file_name() == target_file_name {
					self.current_index = index;
				}
			}

			// Delete all entries that are outside the window of files around the current file
			// allowed by the capacity
			// And there is just one more thing left to do...
			// Walk through our list of directory entries sorted by their distance from the current
			// file and in each step remove an entry from the cache until we reach the desired cache
			// size

			let (mut new_cache, remaining_capacity) = {
				let mut sorted_files: Vec<_> =
					self.texture_cache.iter().enumerate().rev().collect();
				sorted_files.sort_by_key(|(index, _)| {
					(*index as isize - self.current_index as isize).abs()
				});

				let mut remaining_capacity = self.total_capacity;
				//let mut est_file_capacity = self.total_capacity / self.curr_est_size;
				let mut new_cache = BTreeMap::new();
				for (_, (path, texture)) in sorted_files.into_iter() {
					match texture {
						CachedTexture::LoadRequested => {
							new_cache.insert(path.clone(), CachedTexture::LoadRequested);
						}
						CachedTexture::Texture(texture) => {
							// Thew new file has to fit in the cache after this operation
							// which is why we multiply the estimated size by two
							if remaining_capacity > (self.curr_est_size * 2) {
								let dimensions = (texture.1.width(), texture.1.height());
								remaining_capacity -= get_image_size_estimate(dimensions) as isize;
								new_cache
									.insert(path.clone(), CachedTexture::Texture(texture.clone()));
							}
						}
					}
				}

				(new_cache, remaining_capacity)
			};

			mem::swap(&mut self.texture_cache, &mut new_cache);
			self.remaining_capacity = remaining_capacity;
		}

		let metadata = fs::metadata(path.as_path())?;

		// Check if it is inside the texture cache first
		{
			let texture_entry = self.texture_cache.entry(target_file_name.clone());
			if let Entry::Occupied(ref entry) = texture_entry {
				if let CachedTexture::Texture(ref entry) = entry.get() {
					if entry.0.modified().unwrap() == metadata.modified().unwrap() {
						return Ok(entry.1.clone());
					}
				}
			}
		}

		let image = load_image(path.as_path())?;
		self.curr_est_size = get_image_size_estimate((image.width(), image.height())) as isize;
		let image_size_estimate = self.curr_est_size;
		if self.remaining_capacity < image_size_estimate {
			self.texture_cache.clear();
			self.remaining_capacity = self.total_capacity;
		}
		self.remaining_capacity -= image_size_estimate;

		let result_texture = Rc::new(texture_from_image(display, image)?);
		match self.texture_cache.entry(target_file_name) {
			Entry::Vacant(entry) => {
				entry.insert(CachedTexture::Texture((metadata, result_texture.clone())));
			}
			Entry::Occupied(mut entry) => match entry.get_mut() {
				entry @ CachedTexture::LoadRequested => {
					*entry = CachedTexture::Texture((metadata, result_texture.clone()));
				}
				CachedTexture::Texture(ref mut entry) => {
					if entry.0.modified().unwrap() != metadata.modified().unwrap() {
						*entry = (metadata, result_texture.clone());
					}
				}
			},
		}

		Ok(result_texture)
	}

	#[cfg(feature = "networking")]
	pub fn load_url(
		&mut self,
		display: &glium::Display,
		url: &str,
	) -> Result<(Rc<SrgbTexture2d>, OsString)> {
		let client = reqwest::blocking::Client::builder()
			.user_agent("emulsion")
			.build()
			.map_err(|e| Error::from(format!("Could not build client for request: {}", e)))?;

		let response = client
			.get(url)
			.send()
			.map_err(|e| Error::from(format!("Failed to get image: {}", e)))?;

		let bytes = response
			.bytes()
			.map_err(|e| Error::from(format!("Failed to get image content: {}", e)))?;

		// Lets just process incoming images
		self.process_prefetched(display)?;

		let image = load_image_data(&bytes)?;
		self.curr_est_size = get_image_size_estimate((image.width(), image.height())) as isize;
		let image_size_estimate = self.curr_est_size;
		if self.remaining_capacity < image_size_estimate {
			self.texture_cache.clear();
			self.remaining_capacity = self.total_capacity;
		}
		self.remaining_capacity -= image_size_estimate;

		let result_texture = Rc::new(texture_from_image(display, image)?);

		let title = url.trim_start_matches("https://").trim_start_matches("http://");
		let title = match title.find(|c| c == '?' || c == '#') {
			Some(end) => &title[..end],
			None => title,
		};

		Ok((result_texture, OsString::from(title)))
	}

	pub fn load_next(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
		self.load_jump(display, 1)
	}

	pub fn load_prev(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
		self.load_jump(display, -1)
	}

	pub fn load_jump(
		&mut self,
		display: &glium::Display,
		jump_count: i32,
	) -> Result<(Rc<SrgbTexture2d>, OsString)> {
		if jump_count == 0 {
			let filename = self.current_filename();
			return Ok((
				match self.texture_cache.get(&filename) {
					Some(CachedTexture::Texture(ref entry)) => entry.1.clone(),
					_ => bail!(Error::from("Could not find current file in cache.")),
				},
				filename,
			));
		}

		if self.dir_files.is_empty() {
			return Err("Folder is empty or no folder was open when trying to load image.".into());
		}

		let mut target_index =
			(self.current_index as isize + jump_count as isize) % self.dir_files.len() as isize;
		if target_index < 0 {
			target_index += self.dir_files.len() as isize;
		}

		let target_path = self.dir_files[target_index as usize].dir_entry.path();
		let result = self.load_specific(display, &target_path)?;
		self.current_index = target_index as usize;

		Ok((result, target_path.file_name().unwrap_or_else(|| OsStr::new("")).to_owned()))
	}

	pub fn process_prefetched(&mut self, display: &glium::Display) -> Result<()> {
		use std::collections::btree_map::Entry;
		use std::sync::mpsc::TryRecvError;

		let mut processed = 0;
		while processed < 2 {
			match self.loader.try_recv_prefetched() {
				Ok(load_result) => {
					self.requested_images -= 1;
					if let LoadResult::Ok { path, metadata, image } = load_result {
						processed += 1;
						let size_estimate =
							get_image_size_estimate((image.width(), image.height())) as isize;
						match self.texture_cache.entry(path.file_name().unwrap().to_owned()) {
							Entry::Vacant(entry) => {
								let texture = Rc::new(texture_from_image(display, image)?);
								entry.insert(CachedTexture::Texture((metadata, texture)));
								self.remaining_capacity -= size_estimate;
							}
							Entry::Occupied(mut entry) => match entry.get_mut() {
								CachedTexture::Texture(ref mut entry) => {
									if entry.0.modified().unwrap() < metadata.modified().unwrap() {
										let old_size_estimate = {
											let old_image = &entry.1;
											get_image_size_estimate((
												old_image.width(),
												old_image.height(),
											)) as isize
										};
										let texture = Rc::new(texture_from_image(display, image)?);
										entry.0 = metadata;
										entry.1 = texture;
										self.remaining_capacity += old_size_estimate;
										self.remaining_capacity -= size_estimate;
									}
								}
								entry @ CachedTexture::LoadRequested => {
									let texture = Rc::new(texture_from_image(display, image)?);
									*entry = CachedTexture::Texture((metadata, texture));
									self.remaining_capacity -= size_estimate;
								}
							},
						}
					}
				}
				Err(TryRecvError::Disconnected) => return Ok(()),
				Err(TryRecvError::Empty) => break,
			}
		}

		Ok(())
	}

	pub fn prefetch_neighbors(&mut self) {
		let mut index = self.current_index;

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
		use std::collections::btree_map::Entry;

		if self.requested_images >= Self::MAX_PENDING_PREFETCH_REQUESTS {
			return false;
		}

		if self.remaining_capacity > self.curr_est_size as isize {
			if let Some(desc) = self.dir_files.get(index) {
				let file = &desc.dir_entry;
				let file_path = file.path();
				let file_name = if let Some(file_name) = file_path.file_name() {
					file_name.to_owned()
				} else {
					return false;
				};
				match self.texture_cache.entry(file_name) {
					Entry::Vacant(entry) => {
						if is_file_supported(file_path.as_ref()) {
							entry.insert(CachedTexture::LoadRequested);
							self.loader.send_load_request(file_path);
							self.requested_images += 1;
							return true;
						}
					}
					Entry::Occupied(entry) => {
						if let CachedTexture::Texture(ref entry) = entry.get() {
							if entry.0.modified().unwrap()
								!= file.metadata().unwrap().modified().unwrap()
							{
								self.loader.send_load_request(file_path);
								self.requested_images += 1;
							}
						}
						return true;
					}
				}
			}
		}
		false
	}

	fn change_directory(&mut self, dir_path: PathBuf, filename: OsString) -> Result<()> {
		self.dir_files = Self::collect_directory(dir_path.as_path())?;

		// Look up the index of the filename in the directory
		for (index, desc) in self.dir_files.iter().enumerate() {
			if desc.dir_entry.file_name() == filename {
				self.current_index = index;
				self.dir_path = dir_path;
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

	fn collect_directory(path: &Path) -> Result<Vec<ImageDescriptor>> {
		let mut dir_files: Vec<_> = fs::read_dir(path)?
			.filter_map(|x| match x {
				Ok(entry) => match entry.file_type() {
					Ok(file_type) => {
						if file_type.is_file() {
							if is_file_supported(entry.path().as_path()) {
								Some(ImageDescriptor::from_entry(entry))
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
