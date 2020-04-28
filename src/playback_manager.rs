use std::ffi::OsString;
use std::io::Write;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use rand::seq::SliceRandom;
use rand::thread_rng;

use sys_info;

use gelatin::glium;

use gelatin::window::Window;
//use crate::window::Window;

use crate::image_cache::{self, ImageCache, AnimationFrameTexture};

const NANOS_PER_SEC: u64 = 1_000_000_000;

#[derive(Debug, PartialEq)]
pub enum LoadRequest {
	None,
	LoadNext,
	LoadPrevious,
	FilePath(PathBuf),
	LoadAtIndex(usize),
	Jump(i32),
}

#[derive(PartialEq, Copy, Clone)]
pub enum PlaybackState {
	Paused,
	Forward,
	Present,
	RandomPresent,
	//Backward,
}


fn folder_load_next(image_cache: &mut ImageCache, display: &glium::Display) -> FrameLoadResult {
	image_cache.load_next(display)
}
fn folder_load_prev(image_cache: &mut ImageCache, display: &glium::Display) -> FrameLoadResult {
	image_cache.load_prev(display)
}
fn folder_load_jump(image_cache: &mut ImageCache, display: &glium::Display, amount: i32) -> FrameLoadResult {
	image_cache.load_jump(display, amount, 0)
}
fn folder_load_path(image_cache: &mut ImageCache, display: &glium::Display, path: &Path) -> image_cache::Result<AnimationFrameTexture> {
	image_cache.load_specific(display, path, 0)
}
fn folder_load_at_index(image_cache: &mut ImageCache, display: &glium::Display, index: usize) -> FrameLoadResult {
	image_cache.load_at_index(display, index, 0)
}
fn folder_delay_nanos(_player: &ImgSequencePlayer) -> u64 {
	const FRAMERATE: u64 = 25;
	NANOS_PER_SEC / FRAMERATE
}

fn anim_load_next(image_cache: &mut ImageCache, display: &glium::Display) -> FrameLoadResult {
	image_cache.load_jump(display, 0, 1)
}
fn anim_load_prev(image_cache: &mut ImageCache, display: &glium::Display) -> FrameLoadResult {
	image_cache.load_jump(display, 0, -1)
}
fn anim_load_jump(image_cache: &mut ImageCache, display: &glium::Display, amount: i32) -> FrameLoadResult {
	image_cache.load_jump(display, 0, amount as isize)
}
fn anim_load_path(image_cache: &mut ImageCache, display: &glium::Display, path: &Path) -> image_cache::Result<AnimationFrameTexture> {
	image_cache.load_specific(display, path, 0)
}
fn anim_load_at_index(image_cache: &mut ImageCache, display: &glium::Display, index: usize) -> FrameLoadResult {
	image_cache.load_at_index(display, image_cache.current_file_index(), index as isize)
}
fn anim_delay_nanos(player: &ImgSequencePlayer) -> u64 {
	if let Some(ref frame) = player.image_texture {
		frame.delay_nano
	} else {
		0
	}
}


pub struct PlaybackManager {
	//playback_state: PlaybackState,
	image_cache: ImageCache,

	// image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
	// filename: Option<OsString>,

	folder_player: ImgSequencePlayer,
	image_player: ImgSequencePlayer,
}

impl PlaybackManager {
	pub fn new() -> Self {
		let cache_capaxity = match sys_info::mem_info() {
			Ok(value) => {
				// value originally reported in KiB
				((value.total / 8) * 1024) as isize
			}
			_ => {
				println!("Could not get system memory size, using default value");
				// bytes
				500_000_000
			}
		};

		let thread_count = match sys_info::cpu_num() {
			Ok(value) => value.max(2).min(4),
			_ => 4,
		};

		let mut result = PlaybackManager {
			//playback_state: PlaybackState::Paused,
			image_cache: ImageCache::new(cache_capaxity, thread_count),
			folder_player: ImgSequencePlayer::new(
				&folder_load_next,
				&folder_load_prev,
				&folder_load_jump,
				&folder_load_path,
				&folder_load_at_index,
				&folder_delay_nanos,
			),
			image_player: ImgSequencePlayer::new(
				&anim_load_next,
				&anim_load_prev,
				&anim_load_jump,
				&anim_load_path,
				&anim_load_at_index,
				&anim_delay_nanos,
			),
		};
		result.image_player.start_playback_forward();
		result
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
		self.folder_player.start_random_presentation(&self.image_cache);
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

	pub fn current_filename(&self) -> OsString {
		self.image_cache.current_filename()
	}

	pub fn current_file_path(&self) -> PathBuf {
		self.image_cache.current_file_path()
	}

	pub fn current_file_index(&self) -> usize {
		self.image_cache.current_file_index()
	}

	pub fn current_dir_len(&self) -> usize {
		self.image_cache.current_dir_len()
	}

	pub fn update_directory(&mut self) -> image_cache::Result<()> {
		self.image_cache.update_directory()?;
		let index = self.current_file_index();
		self.request_load(LoadRequest::LoadAtIndex(index));
		Ok(())
	}

	pub fn cached_from_dir(&self) -> Vec<bool> {
		self.image_cache.cached_from_dir()
	}

	pub fn request_load(&mut self, request: LoadRequest) {
		self.folder_player.request_load(request);
		self.image_player.request_load(LoadRequest::Jump(0));
	}

	pub fn image_texture(&self) -> Option<Rc<glium::texture::SrgbTexture2d>> {
		self.image_player.image_texture()
	}

	pub fn filename(&self) -> &Option<OsString> {
		&self.folder_player.filename
	}

	pub fn update_image(&mut self, window: &Window) -> gelatin::NextUpdate {
		let display = window.display_mut();
		let prev_file = self.folder_player.image_texture();
		let next_update = self.folder_player.update_image(&display, &mut self.image_cache);
		let new_file = self.folder_player.image_texture();
		if let (Some(prev), Some(new)) = (prev_file, new_file) {
			let file_changed = !Rc::ptr_eq(&prev, &new);
			if file_changed {
				self.image_player.pause_playback();
				self.image_player.start_playback_forward();
			}
		}
		next_update.aggregate(self.image_player.update_image(&display, &mut self.image_cache))
	}
}

type FrameLoadResult = image_cache::Result<(AnimationFrameTexture, OsString)>;
trait LoadHandler {
	fn load_next(&mut self) -> FrameLoadResult;
	fn load_prev(&mut self) -> FrameLoadResult;
	fn load_jump(&mut self, steps: isize) -> FrameLoadResult;
	fn load_path(&mut self, path: &Path) -> FrameLoadResult;
	fn load_at_index(&mut self, index: usize) -> FrameLoadResult;
}
struct ImgSequencePlayer {
	playback_state: PlaybackState,
	present_remaining: Vec<usize>,

	last_frame_change_time: Instant,
	frametime_drift_offset: i64, // in nanosecs
	//frame_count_since_playback_start: u64,

	load_request: LoadRequest,

	image_texture: Option<AnimationFrameTexture>,
	filename: Option<OsString>,

	load_next: &'static dyn Fn(&mut ImageCache, &glium::Display) -> FrameLoadResult,
	load_prev: &'static dyn Fn(&mut ImageCache, &glium::Display) -> FrameLoadResult,
	load_jump: &'static dyn Fn(&mut ImageCache, &glium::Display, i32) -> FrameLoadResult,
	load_path: &'static dyn Fn(&mut ImageCache, &glium::Display, &Path) -> image_cache::Result<AnimationFrameTexture>,
	load_at_index: &'static dyn Fn(&mut ImageCache, &glium::Display, usize) -> FrameLoadResult,

	get_delay_nanos: &'static dyn Fn(&Self) -> u64,
}

impl ImgSequencePlayer {
	pub fn new(
		load_next: &'static dyn Fn(&mut ImageCache, &glium::Display) -> FrameLoadResult,
		load_prev: &'static dyn Fn(&mut ImageCache, &glium::Display) -> FrameLoadResult,
		load_jump: &'static dyn Fn(&mut ImageCache, &glium::Display, i32) -> FrameLoadResult,
		load_path: &'static dyn Fn(&mut ImageCache, &glium::Display, &Path) -> image_cache::Result<AnimationFrameTexture>,
		load_at_index: &'static dyn Fn(&mut ImageCache, &glium::Display, usize) -> FrameLoadResult,
		get_delay_nanos: &'static dyn Fn(&Self) -> u64,
	) -> ImgSequencePlayer {
		ImgSequencePlayer {
			playback_state: PlaybackState::Paused,
			present_remaining: Vec::new(),
			last_frame_change_time: Instant::now(),
			frametime_drift_offset: 0,
			//frame_count_since_playback_start: 0,
			load_request: LoadRequest::None,
			//should_sleep: true,
			image_texture: None,
			filename: None,
			load_next,
			load_prev,
			load_jump,
			load_path,
			load_at_index,
			get_delay_nanos,
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

	pub fn start_random_presentation(&mut self, image_cache: &ImageCache) {
		self.last_frame_change_time = Instant::now();
		self.frametime_drift_offset = 0;
		//self.frame_count_since_playback_start = 0;
		self.playback_state = PlaybackState::RandomPresent;
		self.fill_present_remainig_with_random(image_cache);
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

	pub fn image_texture(&self) -> Option<Rc<glium::texture::SrgbTexture2d>> {
		self.image_texture.clone().map(|t| t.texture)
	}

	pub fn filename(&self) -> &Option<OsString> {
		&self.filename
	}

	pub fn update_image(&mut self, display: &glium::Display, image_cache: &mut ImageCache) -> gelatin::NextUpdate {
		let now = Instant::now();
		let a_millisec_from_now = now.checked_add(Duration::from_millis(1)).unwrap();
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
				frame_delta_time_nanos = (self.get_delay_nanos)(&self) as i64;
			},
		};
		if self.playback_state == PlaybackState::Paused {
			if let Err(e) = image_cache.process_prefetched(display) {
				eprintln!("Failed to process prefetched images with error '{:?}'", e);
			}
			match load_request {
				LoadRequest::Jump(0) => { // Waiting on current image to be loaded. 
					next_update = gelatin::NextUpdate::WaitUntil(a_millisec_from_now);
				}, 
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
				now.checked_add(Duration::from_millis((millis_til_next / 2).max(1) as u64)).unwrap(),
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
							if target == None {
								// Restart
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
				const BUISY_WAIT_TRESHOLD: f32 = 0.8;
				if elapsed_nanos > (frame_delta_time_nanos as f32 * BUISY_WAIT_TRESHOLD) as i64 {
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
			next_update = gelatin::NextUpdate::WaitUntil(a_millisec_from_now);
		}
		match load_request {
			LoadRequest::None | LoadRequest::FilePath(..) => (),
			_ => if image_cache.current_dir_len() == 0 {
				return gelatin::NextUpdate::Latest;
			}
		}
		let load_result = match load_request {
			LoadRequest::LoadNext => Some((self.load_next)(image_cache, display)),
			LoadRequest::LoadPrevious => Some((self.load_prev)(image_cache, display)),
			LoadRequest::FilePath(ref file_path) => {
				Some(if let Some(file_name) = file_path.file_name() {
					let load_path = self.load_path;
					load_path(image_cache, display, file_path.as_ref()).map(|x| (x, OsString::from(file_name)))
				} else {
					Err(String::from("Could not extract filename").into())
				})
			}
			LoadRequest::LoadAtIndex(index) => {
				Some((self.load_at_index)(image_cache, display, index))
			}
			LoadRequest::Jump(jump_count) => {
				Some((self.load_jump)(image_cache, display, jump_count))
			}
			LoadRequest::None => None,
		};
		if let Some(result) = load_result {
			match result {
				Ok((frame, filename)) => {
					self.image_texture = Some(frame);
					self.filename = Some(filename);
				}
				Err(image_cache::errors::Error(
					image_cache::errors::ErrorKind::WaitingOnLoader,
					_,
				)) => {
					// Set the load request to jump in place so that
					// next time we attempt to load this again.
					self.load_request = LoadRequest::Jump(0);
					next_update = gelatin::NextUpdate::WaitUntil(a_millisec_from_now);
				}
				Err(err) => {
					println!("load_result was err stufff");
					self.image_texture = None;
					self.filename = None;
					let stderr = &mut ::std::io::stderr();
					let stderr_errmsg = "Error writing to stderr";
					writeln!(stderr, "Error occured while loading image: {}", err)
						.expect(stderr_errmsg);
					for e in err.iter().skip(1) {
						writeln!(stderr, "... caused by: {}", e).expect(stderr_errmsg);
					}
					if let Some(backtrace) = err.backtrace() {
						writeln!(stderr, "backtrace: {:?}", backtrace).expect(stderr_errmsg);
					}
					writeln!(stderr).expect(stderr_errmsg);
				}
			}
		}
		next_update
	}

	fn fill_present_remainig_with_random(&mut self, image_cache: &ImageCache) {
		self.present_remaining.clear();
		for i in 0..image_cache.current_dir_len() {
			self.present_remaining.push(i);
		}
		let mut rng = thread_rng();
		self.present_remaining.as_mut_slice().shuffle(&mut rng);
	}
}
