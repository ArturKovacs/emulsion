use std::path::PathBuf;
use std::sync::{
	atomic::{AtomicBool, Ordering},
	Arc, Condvar, Mutex,
};

use gelatin::image::imageops::{
	flip_horizontal_in_place, flip_vertical_in_place, rotate180_in_place, rotate270, rotate90,
};

use crate::image_cache::image_loader::{complex_load_image, LoadResult, Orientation};

#[derive(Debug, Clone, Eq, PartialEq)]
enum ClipboardState {
	Pending(PathBuf),
	Succeeded,
	Failed,
}

struct ClipboardRequestHandle {
	run_thread: AtomicBool,
	condvar: Condvar,
	state: Mutex<ClipboardState>,
}

pub struct ClipboardHandler {
	request_handle: Arc<ClipboardRequestHandle>,

	thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl ClipboardHandler {
	pub fn new() -> ClipboardHandler {
		let prev_state = ClipboardState::Succeeded;
		let request_handle = Arc::new(ClipboardRequestHandle {
			run_thread: AtomicBool::new(true),
			condvar: Condvar::new(),
			state: Mutex::new(prev_state),
		});
		let handle = {
			let request_handle = request_handle.clone();
			std::thread::spawn(move || {
				Self::request_handler_thread(request_handle);
			})
		};

		ClipboardHandler { request_handle, thread_handle: Some(handle) }
	}

	pub fn request_copy(&mut self, target: PathBuf) -> bool {
		{
			let mut state = self.request_handle.state.lock().unwrap();
			if let ClipboardState::Pending(..) = &*state {
				return false;
			} else {
				*state = ClipboardState::Pending(target);
			}
		}
		// Notify the condvar after releasing the mutex
		self.request_handle.condvar.notify_one();
		true
	}

	fn request_stop_thread(&self) {
		self.request_handle.run_thread.store(false, Ordering::Release);
		self.request_handle.condvar.notify_one();
	}

	pub fn try_get_result(&self) -> Option<bool> {
		let state = self.request_handle.state.lock().unwrap();
		match &*state {
			ClipboardState::Pending(..) => None,
			ClipboardState::Succeeded => Some(true),
			ClipboardState::Failed => Some(false),
		}
	}

	fn request_handler_thread(request_handle: Arc<ClipboardRequestHandle>) {
		const WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(50);
		let mut clipboard = arboard::Clipboard::new();
		if let Err(e) = &clipboard {
			eprintln!("The clipboard could not be created, error was: {}", e);
		}
		while request_handle.run_thread.load(Ordering::Acquire) {
			let request_path;
			{
				let mut state_guard = request_handle.state.lock().unwrap();
				'wait_for_request: loop {
					if let ClipboardState::Pending(path) = state_guard.clone() {
						request_path = path;
						break 'wait_for_request;
					} else {
						if !request_handle.run_thread.load(Ordering::Acquire) {
							return;
						}
						match request_handle.condvar.wait_timeout(state_guard, WAIT_TIMEOUT) {
							Ok((guard, _)) => {
								state_guard = guard;
							}
							Err(e) => {
								panic!("{}", e);
							}
						}
					}
				}
			}
			let result = complex_load_image(&request_path, false, 0, |frame| {
				if let LoadResult::Frame { mut image, orientation, .. } = frame {
					if let Ok(clipboard) = &mut clipboard {
						// Note: the imageops functions use clockwise rotation whereas the
						// `Orientation` type describes counter-clockwise rotation.
						image = match orientation {
							Orientation::Deg0 => image,
							Orientation::Deg0HorFlip => {
								flip_horizontal_in_place(&mut image);
								image
							}
							Orientation::Deg90 => rotate270(&image),
							Orientation::Deg90VerFlip => {
								let mut result = rotate270(&image);
								flip_vertical_in_place(&mut result);
								result
							}
							Orientation::Deg180 => {
								rotate180_in_place(&mut image);
								image
							}
							Orientation::Deg180HorFlip => {
								// This is identical to just a vertical flip with no rotation.
								flip_vertical_in_place(&mut image);
								image
							}
							Orientation::Deg270 => rotate90(&image),
							Orientation::Deg270VerFlip => {
								let mut result = rotate90(&image);
								flip_vertical_in_place(&mut result);
								result
							}
						};
						let (w, h) = image.dimensions();
						let cb_image = arboard::ImageData {
							width: w as usize,
							height: h as usize,
							bytes: image.into_raw().into(),
						};
						if let Err(e) = clipboard.set_image(cb_image) {
							eprintln!("Could not set the clipboard image, error was: {}", e);
						} else {
							return Ok(());
						}
					}
				}
				Err("Could not set the clipboard image.".into())
			});
			let mut state = request_handle.state.lock().unwrap();
			*state =
				if result.is_ok() { ClipboardState::Succeeded } else { ClipboardState::Failed };
		}
	}
}

impl Default for ClipboardHandler {
	fn default() -> Self {
		ClipboardHandler::new()
	}
}

impl Drop for ClipboardHandler {
	fn drop(&mut self) {
		if let Some(handle) = self.thread_handle.take() {
			self.request_stop_thread();
			handle.join().unwrap();
		}
	}
}
