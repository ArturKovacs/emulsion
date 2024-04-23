use std::{
	collections::hash_map::HashMap,
	fmt::Debug,
	rc::Rc,
	sync::atomic::{AtomicBool, Ordering},
	time::{Duration, Instant},
};

use winit::{
	event::{Event, StartCause, WindowEvent},
	event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
	window::WindowId,
};

use crate::{window::Window, NextUpdate};

// const MAX_SLEEP_DURATION: std::time::Duration = std::time::Duration::from_millis(4);
static EXIT_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn request_exit() {
	EXIT_REQUESTED.store(true, Ordering::Relaxed);
}

fn set_control_flow<E>(event_loop: &EventLoopWindowTarget<E>, control_flow: ControlFlow) {
	if let ControlFlow::WaitUntil(time) = control_flow {
		let very_short_time_from_now = Instant::now() + Duration::from_micros(100);
		if time < very_short_time_from_now {
			event_loop.set_control_flow(ControlFlow::Poll);
		} else {
			event_loop.set_control_flow(control_flow);
		}
	} else {
		event_loop.set_control_flow(control_flow);
	}
}

/// Returns true if original was replaced by new
fn aggregate_control_flow<E>(event_loop: &EventLoopWindowTarget<E>, new: ControlFlow) -> bool {
	let original = event_loop.control_flow();
	match new {
		ControlFlow::Poll => {
			set_control_flow(event_loop, new);
			return true;
		}
		ControlFlow::WaitUntil(new_time) => match original {
			ControlFlow::WaitUntil(orig_time) => {
				if new_time < orig_time {
					set_control_flow(event_loop, new);
					return true;
				}
			}
			ControlFlow::Wait => {
				set_control_flow(event_loop, new);
				return true;
			}
			_ => (),
		},
		_ => (),
	}
	false
}

/// On Windows, there's a bug that causes the event loop to get stuck in a
/// repeated "Poll-like" loop with the control flow set to WaitUntil but the
/// loops keeps running iterations immediately after each other.
///
/// We use this function to check if the time specified in WaitUntil has already
/// been passed and if it is, then we change the control flow, to get "un-stuck"
fn sanitize_control_flow<E>(event_loop: &EventLoopWindowTarget<E>) {
	set_control_flow(event_loop, event_loop.control_flow());
}

pub type EventHandler<UserEvent> = dyn FnMut(&Event<UserEvent>) -> NextUpdate;

pub struct Application<UserEvent>
where
	UserEvent: Debug + 'static,
{
	pub event_loop: EventLoop<UserEvent>,
	windows: HashMap<WindowId, Rc<Window>>,
	global_handlers: Vec<Box<EventHandler<UserEvent>>>,
	at_exit: Option<Box<dyn FnOnce()>>,
}

impl<UserEvent> Application<UserEvent>
where
	UserEvent: Debug + 'static,
{
	pub fn new() -> Self {
		Application {
			event_loop: EventLoopBuilder::<UserEvent>::with_user_event().build().unwrap(),
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

	pub fn add_global_event_handler<F: FnMut(&Event<UserEvent>) -> NextUpdate + 'static>(
		&mut self,
		fun: F,
	) {
		self.global_handlers.push(Box::new(fun));
	}

	pub fn create_loop_proxy(&self) -> EventLoopProxy<UserEvent> {
		self.event_loop.create_proxy()
	}

	pub fn start_event_loop(self) {
		let mut windows: HashMap<WindowId, Rc<Window>> = self.windows;
		let mut at_exit = self.at_exit;
		let mut global_handlers = self.global_handlers;
		#[cfg(feature = "benchmark")]
		let mut last_draw_time = std::time::Instant::now();
		#[cfg(feature = "benchmark")]
		let mut prev_draw_dts = vec![0f32; 64];
		#[cfg(feature = "benchmark")]
		let mut prev_draw_dt_index = 0;
		#[cfg(feature = "benchmark")]
		let mut update_draw_dt = move || {
			let now = std::time::Instant::now();
			let delta_time = now.duration_since(last_draw_time).as_secs_f32();
			last_draw_time = now;
			prev_draw_dts[prev_draw_dt_index] = delta_time;
			prev_draw_dt_index = (prev_draw_dt_index + 1) % prev_draw_dts.len();
			if prev_draw_dt_index == 0 {
				let max_dt = prev_draw_dts.iter().fold(0.0f32, |a, &b| a.max(b));
				println!(
					"{} redraws finsished, max delta time in that duration was: {}ms, {} FPS",
					prev_draw_dts.len(),
					(max_dt * 1000.0).round() as i32,
					(1.0 / max_dt).round() as i32
				);
			}
		};

		self.event_loop
			.run(move |event, event_loop| {
				sanitize_control_flow(event_loop);
				for handler in global_handlers.iter_mut() {
					let handler_next_update = handler(&event);
					aggregate_control_flow(event_loop, handler_next_update.into());
				}
				// dbg!(&event);
				match event {
					Event::NewEvents(start_cause) => {
						if start_cause == StartCause::Init {
							event_loop.set_control_flow(ControlFlow::Wait);
						}
						for window in windows.values() {
							let new_control_flow = window.handle_loop_wake_up().into();
							aggregate_control_flow(event_loop, new_control_flow);
						}
					}
					Event::WindowEvent { event, window_id } => {
						if let WindowEvent::RedrawRequested = event {
							let new_control_flow = windows.get(&window_id).unwrap().redraw().into();
							aggregate_control_flow(event_loop, new_control_flow);
							#[cfg(feature = "benchmark")]
							update_draw_dt();
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
						windows.get(&window_id).unwrap().process_event(event, event_loop);
						if destroyed {
							windows.remove(&window_id);
						}
					}
					Event::AboutToWait => {
						if EXIT_REQUESTED.load(Ordering::Relaxed) {
							event_loop.exit();
							return;
						} else {
							for window in windows.values() {
								window.main_events_cleared();
								if window.redraw_needed() {
									window.request_redraw();
								}
							}
							event_loop.set_control_flow(ControlFlow::Wait);
						}
					}
					Event::LoopExiting => {
						if let Some(at_exit) = at_exit.take() {
							at_exit();
						}
					}
					event => {
						log::trace!("Ignoring event: {event:?}");
					}
				}

				#[cfg(all(unix, not(target_os = "macos")))]
				if matches!(control_flow, ControlFlow::Poll) {
					const MAX_SLEEP_DURATION: std::time::Duration =
						std::time::Duration::from_millis(4);

					// This is an ugly workaround for the X server completely freezing
					// sometimes.
					// See: https://github.com/ArturKovacs/emulsion/issues/172
					let now = std::time::Instant::now();
					event_loop.set_control_flow(ControlFlow::WaitUntil(now + MAX_SLEEP_DURATION));
				}
			})
			.unwrap();
	}
}

impl<UserEvent> Default for Application<UserEvent>
where
	UserEvent: Debug + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}
