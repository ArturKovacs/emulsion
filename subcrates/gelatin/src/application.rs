use std::collections::hash_map::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use glium::glutin::{
	self,
	event::{Event, WindowEvent},
	event_loop::ControlFlow,
	window::WindowId,
};

use crate::window::Window;
use crate::NextUpdate;

const MAX_SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(4);
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn request_exit() {
	EXIT_REQUESTED.store(true, Ordering::Relaxed);
}

/// Returns true if original was replaced by new
fn aggregate_control_flow(original: &mut ControlFlow, new: ControlFlow) -> bool {
	if *original == ControlFlow::Exit {
		return false;
	}
	match new {
		ControlFlow::Exit | ControlFlow::Poll => {
			*original = new;
			return true;
		}
		ControlFlow::WaitUntil(new_time) => match *original {
			ControlFlow::WaitUntil(orig_time) => {
				if new_time < orig_time {
					*original = new;
					return true;
				}
			}
			ControlFlow::Wait => {
				*original = new;
				return true;
			}
			_ => (),
		},
		_ => (),
	}
	false
}

fn update_control_flow(
	prev_control_flow_source: &mut WindowId,
	new_control_flow_source: WindowId,
	control_flow: &mut ControlFlow,
	new_control_flow: ControlFlow,
) {
	if *prev_control_flow_source == new_control_flow_source {
		*control_flow = new_control_flow;
	} else if *control_flow != ControlFlow::Exit
		&& aggregate_control_flow(control_flow, new_control_flow)
	{
		*prev_control_flow_source = new_control_flow_source;
	}
}

pub struct Application {
	pub event_loop: glutin::event_loop::EventLoop<()>,
	windows: HashMap<WindowId, Rc<Window>>,
	global_handlers: Vec<Box<dyn FnMut(&Event<()>) -> NextUpdate>>,
	at_exit: Option<Box<dyn FnOnce()>>,
}

impl Application {
	pub fn new() -> Application {
		Application {
			event_loop: glutin::event_loop::EventLoop::<()>::new(),
			windows: HashMap::new(),
			global_handlers: Vec::new(),
			at_exit: None,
		}
	}

	pub fn set_at_exit<F: FnOnce() + 'static>(&mut self, fun: Option<F>) {
		match fun {
			Some(fun) => self.at_exit = Some(Box::new(fun)),
			None => self.at_exit = None,
		};
	}

	pub fn register_window(&mut self, window: Rc<Window>) {
		self.windows.insert(window.get_id(), window);
	}

	pub fn add_global_event_handler<F: FnMut(&Event<()>) -> NextUpdate + 'static>(
		&mut self,
		fun: F,
	) {
		self.global_handlers.push(Box::new(fun));
	}

	pub fn start_event_loop(self) -> ! {
		let mut windows = self.windows;
		let mut at_exit = self.at_exit;
		let mut global_handlers = self.global_handlers;
		let mut control_flow_source = *windows.keys().next().unwrap();
		self.event_loop.run(move |event, _event_loop, control_flow| {
			for handler in global_handlers.iter_mut() {
				aggregate_control_flow(control_flow, handler(&event).into());
			}
			match event {
				Event::WindowEvent { event, window_id } => {
					if let WindowEvent::Resized { .. } = event {
						windows.get(&window_id).unwrap().request_redraw();
					}
					if let WindowEvent::CloseRequested = event {
						// This actually wouldn't be okay for a general pupose ui toolkit,
						// but gelatin is specifically made for emulsion so this is fine hehe
						request_exit();
					}
					let destroyed;
					if let WindowEvent::Destroyed = event {
						destroyed = true;
					} else {
						destroyed = false;
					}
					windows.get(&window_id).unwrap().process_event(event);
					if destroyed {
						windows.remove(&window_id);
					}
				}
				Event::MainEventsCleared => {
					if !EXIT_REQUESTED.load(Ordering::Relaxed) {
						let mut should_sleep = !matches!(control_flow, ControlFlow::Poll);
						for (window_id, window) in windows.iter() {
							let new_control_flow = window.main_events_cleared().into();
							update_control_flow(
								&mut control_flow_source,
								*window_id,
								control_flow,
								new_control_flow,
							);
							should_sleep = should_sleep && window.should_sleep();
							if window.redraw_needed() {
								window.request_redraw();
							}
						}
						if should_sleep {
							let mut sleep_duration = MAX_SLEEP_DURATION;
							if let ControlFlow::WaitUntil(next_update) = control_flow {
								let now = std::time::Instant::now();
								if *next_update > now {
									let control_flow_sleep = *next_update - now;
									if control_flow_sleep < sleep_duration {
										sleep_duration = control_flow_sleep;
									}
								} else {
									should_sleep = false;
								}
							}
							if should_sleep {
								std::thread::sleep(sleep_duration);
							}
						}
					} else {
						*control_flow = ControlFlow::Exit;
					}
				}
				Event::RedrawRequested(window_id) => {
					let new_control_flow = windows.get(&window_id).unwrap().redraw().into();
					update_control_flow(
						&mut control_flow_source,
						window_id,
						control_flow,
						new_control_flow,
					);
				}
				Event::RedrawEventsCleared => {
					if EXIT_REQUESTED.load(Ordering::Relaxed) {
						*control_flow = ControlFlow::Exit;
					}
				}
				_ => {
					*control_flow = ControlFlow::Wait;
				}
			}
			if *control_flow == ControlFlow::Exit {
				if let Some(at_exit) = at_exit.take() {
					at_exit();
				}
				// Drop 'em all!
				//windows.clear();
			}
		});
	}
}

impl Default for Application {
	fn default() -> Self {
		Self::new()
	}
}
