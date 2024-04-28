use std::io::Write;
use std::marker::PhantomData;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use rand::thread_rng;

use log::{debug, trace};

use gelatin::window::Window;
use gelatin::Display;

use crate::image_cache::{
	self, AnimationFrameTexture, ImageCache, LoadResult2, PathResolutionError, TextureResult,
};

use image_cache::directory;

const NANOS_PER_SEC: u64 = 1_000_000_000;

#[derive(Debug, Eq, PartialEq)]
pub enum LoadRequest {
	None,
	LoadNext,
	LoadPrevious,
	FilePath(PathBuf),
	LoadAtIndex(usize),
	Jump(i32),
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum PlaybackState {
	Paused,
	Forward,
	Present,
	RandomPresent,
	//Backward,
}

trait Playback: Sized {
	fn load_next(image_cache: &mut ImageCache, display: &Display) -> LoadResult2;

	fn load_prev(image_cache: &mut ImageCache, display: &Display) -> LoadResult2;

	fn load_jump(image_cache: &mut ImageCache, display: &Display, amount: i32) -> LoadResult2;

	fn load_path(
		image_cache: &mut ImageCache,
		display: &Display,
		path: &Path,
	) -> TextureResult<AnimationFrameTexture> {
		image_cache.load_specific(display, path, None)
	}

	fn load_at_index(image_cache: &mut ImageCache, display: &Display, index: usize) -> LoadResult2;

	fn delay_nanos(player: &ImgSequencePlayer<Self>) -> u64;
}

struct FolderPlayback;

impl Playback for FolderPlayback {
	fn load_next(image_cache: &mut ImageCache, display: &Display) -> LoadResult2 {
		image_cache.load_next(display)
	}

	fn load_prev(image_cache: &mut ImageCache, display: &Display) -> LoadResult2 {
		image_cache.load_prev(display)
	}

	fn load_jump(image_cache: &mut ImageCache, display: &Display, amount: i32) -> LoadResult2 {
		image_cache.load_jump(display, amount, 0)
	}

	fn load_at_index(image_cache: &mut ImageCache, display: &Display, index: usize) -> LoadResult2 {
		image_cache.load_at_index(display, index, None)
	}

	fn delay_nanos(_player: &ImgSequencePlayer<Self>) -> u64 {
		const FRAMERATE: u64 = 25;
		NANOS_PER_SEC / FRAMERATE
	}
}

struct AnimPlayback;

impl Playback for AnimPlayback {
	fn load_next(image_cache: &mut ImageCache, display: &Display) -> LoadResult2 {
		image_cache.load_jump(display, 0, 1)
	}

	fn load_prev(image_cache: &mut ImageCache, display: &Display) -> LoadResult2 {
		image_cache.load_jump(display, 0, -1)
	}

	fn load_jump(image_cache: &mut ImageCache, display: &Display, amount: i32) -> LoadResult2 {
		image_cache.load_jump(display, 0, amount as isize)
	}

	fn load_at_index(image_cache: &mut ImageCache, display: &Display, index: usize) -> LoadResult2 {
		if let Some(curr_index) = image_cache.current_file_index() {
			image_cache.load_at_index(display, curr_index, Some(index as isize))
		} else {
			Err(PathResolutionError::WaitingOnDirFilter)
		}
	}

	fn delay_nanos(player: &ImgSequencePlayer<Self>) -> u64 {
		if let Some(ref frame) = player.image_texture {
			frame.delay_nano
		} else {
			0
		}
	}
}

pub struct PlaybackManager {
	//playback_state: PlaybackState,
	image_cache: ImageCache,

	// image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
	// filename: Option<OsString>,
	folder_player: ImgSequencePlayer<FolderPlayback>,
	image_player: ImgSequencePlayer<AnimPlayback>,
}

impl PlaybackManager {
	pub fn new() -> Self {
		let cache_capaxity = match sys_info::mem_info() {
			Ok(value) => {
				// value originally reported in KiB
				((value.total / 8) * 1024) as isize
			}
			_ => {
				eprintln!("Could not get system memory size, using default value");
				// bytes
				500_000_000
			}
		};

		let thread_count = match sys_info::cpu_num() {
			Ok(value) => value.clamp(2, 4),
			_ => 4,
		};

		PlaybackManager {
			//playback_state: PlaybackState::Paused,
			image_cache: ImageCache::new(cache_capaxity, thread_count),
			folder_player: ImgSequencePlayer::new(),
			image_player: ImgSequencePlayer::new(),
		}
	}

	pub fn playback_state(&self) -> PlaybackState {
		self.folder_player.playback_state()
	}

	pub fn start_playback_forward(&mut self) {
		self.folder_player.start_playback_forward();
		// self.playback_start_time = Instant::now();
		// self.frame_count_since_playback_start = 0;
		// self.playback_state = PlaybackState::Forward;
	}

	pub fn pause_playback(&mut self) {
		self.folder_player.pause_playback();
		//self.playback_state = PlaybackState::Paused;
	}

	pub fn start_random_presentation(&mut self) {
		self.folder_player.start_random_presentation(&mut self.image_cache);
		//self.playback_start_time = Instant::now();
		//self.frame_count_since_playback_start = 0;
		//self.playback_state = PlaybackState::RandomPresent;
		//self.fill_present_remainig_with_random();
	}

	pub fn start_presentation(&mut self) {
		self.folder_player.start_presentation();
		// self.playback_start_time = Instant::now();
		// self.frame_count_since_playback_start = 0;
		// self.playback_state = PlaybackState::Present;
	}

	/// Returns None when the folder hasn't finished filtering
	pub fn current_file_index(&mut self) -> Option<usize> {
		self.image_cache.current_file_index()
	}

	/// Returns None when the folder hasn't finished filtering
	pub fn current_dir_len(&mut self) -> Option<usize> {
		self.image_cache.current_dir_len()
	}

	pub fn update_directory(&mut self) -> directory::Result<()> {
		debug!("In `update_directory`");
		if let LoadRequest::None = self.folder_player.load_request {
			let curr_path = self.image_cache.current_file_path();
			debug!("In `update_directory`, current_file_path is: {:?}", curr_path);
			if curr_path.is_some() {
				self.image_cache.update_directory()?;

				// The there's no file to open, just request to open the empty path.
				// This will hide the previously loaded image.
				// Note that `image_cache.current_file_path()` is used instead of `self.shown_file_path()`
				let path = self.image_cache.current_file_path().unwrap_or_default();
				self.request_load(LoadRequest::FilePath(path));
			}
		}
		Ok(())
	}

	pub fn request_load(&mut self, request: LoadRequest) {
		self.folder_player.request_load(request);
		self.image_player.request_load(LoadRequest::Jump(0));
	}

	pub fn image_texture(&self) -> Option<AnimationFrameTexture> {
		self.image_player.image_texture()
	}

	/// The path to the image file which is currently rendered onto the screen.
	pub fn shown_file_path(&self) -> &LoadedImgPath {
		&self.folder_player.file_path
	}

	pub fn update_image(&mut self, window: &Window) -> gelatin::NextUpdate {
		let display = window.display_mut();
		let prev_file = self.folder_player.image_texture();
		let next_update = self.folder_player.update_image(&display, &mut self.image_cache);
		trace!("Folder player next update: {:?}", next_update);
		let new_file = self.folder_player.image_texture();
		let mut file_changed = prev_file.is_none() != new_file.is_none();
		if let (Some(prev), Some(new)) = (prev_file, new_file) {
			file_changed = !Rc::ptr_eq(&prev.tex_grid, &new.tex_grid);
		}
		if file_changed {
			self.image_player.start_playback_forward();
			self.image_player.request_load(LoadRequest::Jump(0));
		}
		if self.image_cache.loaded_still_image() {
			self.image_player.pause_playback();
		}
		let img_player_next_update =
			self.image_player.update_image(&display, &mut self.image_cache);
		trace!("Image player next update: {:?}", img_player_next_update);
		next_update.aggregate(img_player_next_update)
	}
}

#[derive(Debug, Clone)]
pub enum LoadedImgPath {
	NotYetLoaded,
	ErrLoading(PathBuf),
	Loaded(PathBuf),
}

impl LoadedImgPath {
	fn is_loaded(&self) -> bool {
		matches!(self, LoadedImgPath::Loaded(_))
	}
}

struct ImgSequencePlayer<P: Playback> {
	playback_state: PlaybackState,
	present_remaining: Vec<usize>,

	last_frame_change_time: Instant,
	frametime_drift_offset: i64, // in nanosecs
	//frame_count_since_playback_start: u64,
	load_request: LoadRequest,

	image_texture: Option<AnimationFrameTexture>,
	file_path: LoadedImgPath,

	_playback: PhantomData<P>,
}

impl<P: Playback> ImgSequencePlayer<P> {
	pub fn new() -> Self {
		ImgSequencePlayer {
			playback_state: PlaybackState::Paused,
			present_remaining: Vec::new(),
			last_frame_change_time: Instant::now(),
			frametime_drift_offset: 0,
			//frame_count_since_playback_start: 0,
			load_request: LoadRequest::None,
			image_texture: None,
			file_path: LoadedImgPath::NotYetLoaded,

			_playback: PhantomData,
		}
	}

	pub fn playback_state(&self) -> PlaybackState {
		self.playback_state
	}

	pub fn start_playback_forward(&mut self) {
		self.last_frame_change_time = Instant::now();
		self.frametime_drift_offset = 0;
		//self.frame_count_since_playback_start = 0;
		self.playback_state = PlaybackState::Forward;
	}

	pub fn pause_playback(&mut self) {
		self.playback_state = PlaybackState::Paused;
	}

	// Returns false if the directory hasn't finished filtering
	pub fn start_random_presentation(&mut self, image_cache: &mut ImageCache) -> bool {
		self.last_frame_change_time = Instant::now();
		self.frametime_drift_offset = 0;
		//self.frame_count_since_playback_start = 0;
		self.playback_state = PlaybackState::RandomPresent;
		self.fill_present_remainig_with_random(image_cache)
	}

	pub fn start_presentation(&mut self) {
		self.last_frame_change_time = Instant::now();
		self.frametime_drift_offset = 0;
		//self.frame_count_since_playback_start = 0;
		self.playback_state = PlaybackState::Present;
	}

	pub fn request_load(&mut self, request: LoadRequest) {
		self.load_request = request;
	}

	pub fn image_texture(&self) -> Option<AnimationFrameTexture> {
		self.image_texture.clone()
	}

	pub fn update_image(
		&mut self,
		display: &Display,
		image_cache: &mut ImageCache,
	) -> gelatin::NextUpdate {
		trace!(
			"Begin `update_image`. Curr image is: {:?}. Load request is {:?}",
			self.file_path,
			self.load_request
		);
		let is_paused = matches!(self.playback_state, PlaybackState::Paused);
		let no_request = matches!(self.load_request, LoadRequest::None);
		if !self.file_path.is_loaded() && no_request && is_paused {
			return gelatin::NextUpdate::Latest;
		}
		let now = Instant::now();
		let few_millisecs_from_now = now.checked_add(Duration::from_millis(50)).unwrap();
		let mut next_update;
		// The reason why I reset the `self.load_request` in such a convoluted way is that
		// it has to be guaranteed that it will be reset even if I return from this
		// function early. And at the same time I want to use it's value as it is at this line.
		let mut load_request = LoadRequest::None;
		mem::swap(&mut self.load_request, &mut load_request);
		let frame_delta_time_nanos;
		match self.playback_state {
			PlaybackState::Present | PlaybackState::RandomPresent => {
				frame_delta_time_nanos = (NANOS_PER_SEC * 6) as i64;
			}
			_ => {
				frame_delta_time_nanos = P::delay_nanos(self) as i64;
			}
		};
		if self.playback_state == PlaybackState::Paused {
			if let Err(e) = image_cache.process_prefetched(display) {
				eprintln!("Failed to process prefetched images with error '{:?}'", e);
			}
			match load_request {
				LoadRequest::Jump(0) => {
					// Waiting on current image to be loaded.
					next_update = gelatin::NextUpdate::WaitUntil(few_millisecs_from_now);
				}
				_ => {
					image_cache.prefetch_neighbors();
					next_update = gelatin::NextUpdate::Latest;
				}
			}
		} else if load_request == LoadRequest::None {
			let elapsed = self.last_frame_change_time.elapsed();
			let elapsed_nanos = elapsed.as_secs() * NANOS_PER_SEC + elapsed.subsec_nanos() as u64;
			let elapsed_nanos = elapsed_nanos as i64 + self.frametime_drift_offset;

			let nanos_til_next = frame_delta_time_nanos - elapsed_nanos;
			let millis_til_next = nanos_til_next / 1_000_000;
			next_update = gelatin::NextUpdate::WaitUntil(
				now.checked_add(Duration::from_millis((millis_til_next / 2).max(1) as u64))
					.unwrap(),
			);
			// This assumes that the following frames have the same delay but that's okay considering that
			// if frame step is greater than 1 it almost certainly means that we couldn't load the
			// next frame quiclky enough so there's not much else to do here.
			let frame_step;
			if frame_delta_time_nanos > 0 {
				frame_step = elapsed_nanos / frame_delta_time_nanos;
			} else {
				frame_step = 0;
			}
			if frame_step > 0 {
				load_request = match self.playback_state {
					PlaybackState::Forward | PlaybackState::Present => {
						// if we can't load the frames quickly enough,
						// we won't jump over frames, but instead play the animation slower.
						LoadRequest::Jump(frame_step.min(1) as i32)
					}
					PlaybackState::RandomPresent => {
						let mut target = None;
						for _ in 0..frame_step {
							target = self.present_remaining.pop();
							if target.is_none() {
								// Restart
								// WARNING we silently assume that the folder is fully
								// filtered at this point.
								self.fill_present_remainig_with_random(image_cache);
								target = self.present_remaining.pop();
							}
						}
						match target {
							Some(index) => LoadRequest::LoadAtIndex(index),
							None => LoadRequest::None,
						}
					}
					PlaybackState::Paused => unreachable!(),
				};
				self.last_frame_change_time = Instant::now();
				self.frametime_drift_offset = -nanos_til_next;
			} else {
				image_cache.process_prefetched(display).unwrap();
				const BUISY_WAIT_THRESHOLD: f32 = 0.8;
				if elapsed_nanos > (frame_delta_time_nanos as f32 * BUISY_WAIT_THRESHOLD) as i64 {
					// Just buisy wait if we are getting very close to the next frame swap
					next_update = gelatin::NextUpdate::Soonest;
				} else {
					match self.playback_state {
						PlaybackState::RandomPresent => {
							if let Some(&last) = self.present_remaining.iter().last() {
								image_cache.prefetch_at_index(last);
							}
						}
						_ => image_cache.prefetch_neighbors(),
					}
				}
			}
		} else {
			next_update = gelatin::NextUpdate::WaitUntil(few_millisecs_from_now);
		}
		match load_request {
			LoadRequest::None | LoadRequest::FilePath(..) => (),
			_ => {
				if image_cache.current_dir_len() == Some(0) {
					return gelatin::NextUpdate::Latest;
				}
			}
		}
		trace!("Attempting actual load in `update_image`");
		let load_result = match load_request {
			LoadRequest::LoadNext => Some(P::load_next(image_cache, display)),
			LoadRequest::LoadPrevious => Some(P::load_prev(image_cache, display)),
			LoadRequest::FilePath(file_path) => {
				let load_result = P::load_path(image_cache, display, &file_path);
				Some(Ok((file_path, load_result)))
			}
			LoadRequest::LoadAtIndex(index) => Some(P::load_at_index(image_cache, display, index)),
			LoadRequest::Jump(jump_count) => Some(P::load_jump(image_cache, display, jump_count)),
			LoadRequest::None => None,
		};
		if let Some(loaded_image) = load_result {
			match loaded_image {
				Ok((path, result)) => match result {
					Ok(frame) => {
						self.image_texture = Some(frame);
						self.file_path = LoadedImgPath::Loaded(path);
					}
					Err(image_cache::texture_load_errors::Error(
						image_cache::texture_load_errors::ErrorKind::WaitingOnLoader,
						_,
					)) => {
						// Set the load request to jump in place so that
						// next time we attempt to load this again.
						self.load_request = LoadRequest::Jump(0);
						next_update = gelatin::NextUpdate::WaitUntil(few_millisecs_from_now);
					}
					Err(err) => {
						self.image_texture = None;
						self.file_path = LoadedImgPath::ErrLoading(path);
						let stderr = &mut ::std::io::stderr();
						let stderr_errmsg = "Error writing to stderr";
						writeln!(stderr, "Error occurred while loading image: {}", err)
							.expect(stderr_errmsg);
						for e in err.iter().skip(1) {
							writeln!(stderr, "... caused by: {}", e).expect(stderr_errmsg);
						}
						if let Some(backtrace) = err.backtrace() {
							writeln!(stderr, "backtrace: {:?}", backtrace).expect(stderr_errmsg);
						}
						writeln!(stderr).expect(stderr_errmsg);
					}
				},
				Err(PathResolutionError::WaitingOnDirFilter) => {
					// Set the load request to jump in place so that
					// next time we attempt to load this again.
					self.load_request = LoadRequest::Jump(0);
					next_update = gelatin::NextUpdate::WaitUntil(few_millisecs_from_now);
				}
				Err(PathResolutionError::NotYetSpecified) => {
					self.image_texture = None;
					self.file_path = LoadedImgPath::NotYetLoaded;
				}
			}
		}
		next_update
	}

	fn fill_present_remainig_with_random(&mut self, image_cache: &mut ImageCache) -> bool {
		self.present_remaining.clear();
		if let Some(dir_len) = image_cache.current_dir_len() {
			for i in 0..dir_len {
				self.present_remaining.push(i);
			}
			let mut rng = thread_rng();
			self.present_remaining.as_mut_slice().shuffle(&mut rng);
			true
		} else {
			false
		}
	}
}
