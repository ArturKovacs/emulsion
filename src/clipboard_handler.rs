use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};

use arboard;

use crate::image_cache::image_loader::{complex_load_image, LoadResult};

#[derive(Debug, Clone, Eq, PartialEq)]
enum ClipboardState {
	Pending(PathBuf),
	Succeeded,
	Failed,
}

struct ClipboardRequestHandle {
	condvar: Condvar,
	state: Mutex<ClipboardState>,
}

pub struct ClipboardHandler {
	prev_state: ClipboardState,
	request_handle: Arc<ClipboardRequestHandle>,

	thread_handle: std::thread::JoinHandle<()>,
}

impl ClipboardHandler {
	pub fn new() -> ClipboardHandler {
		let prev_state = ClipboardState::Succeeded;
		let request_handle = Arc::new(ClipboardRequestHandle {
			condvar: Condvar::new(),
			state: Mutex::new(prev_state.clone()),
		});

		let handle = {
			let request_handle = request_handle.clone();
			std::thread::spawn(move || {
				Self::request_handler_thread(request_handle);
			})
		};

		ClipboardHandler { prev_state, request_handle, thread_handle: handle }
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

	pub fn requests_pending(&self) -> bool {
		let mut state = self.request_handle.state.lock().unwrap();
		if let ClipboardState::Pending(..) = &*state {
			true
		} else {
			false
		}
	}

	pub fn try_get_result(&self) -> Option<bool> {
		let mut state = self.request_handle.state.lock().unwrap();
		match &*state {
			ClipboardState::Pending(..) => None,
			ClipboardState::Succeeded => Some(true),
			ClipboardState::Failed => Some(false),
		}
	}

	fn request_handler_thread(request_handle: Arc<ClipboardRequestHandle>) {
		let mut clipboard = arboard::Clipboard::new();
		if let Err(e) = &clipboard {
			eprintln!("The clipboard could not be created, error was: {}", e);
		}
		//let mut prev_state = request_handle.state.lock().unwrap().clone();
		loop {
			let request_path;
			{
				let mut state_guard = request_handle.state.lock().unwrap();
				'wait_for_request: loop {
					if let ClipboardState::Pending(path) = state_guard.clone() {
						request_path = path;
						break 'wait_for_request;
					} else {
						match request_handle.condvar.wait(state_guard) {
							Ok(guard) => {
								state_guard = guard;
							}
							Err(e) => {
								panic!(format!("{}", e));
							}
						}
					}
				}
			}
			if request_path.as_os_str().is_empty() {
				return;
			}
			let result = complex_load_image(&request_path, false, 0, |frame| {
				if let LoadResult::Frame { image, angle, .. } = frame {
					if let Ok(clipboard) = &mut clipboard {
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
			let mut state_guard = request_handle.state.lock().unwrap();
			*state_guard =
				if result.is_ok() { ClipboardState::Succeeded } else { ClipboardState::Failed };
		}
	}
}

impl Default for ClipboardHandler {
	fn default() -> Self {
		ClipboardHandler::new()
	}
}
