use std::rc::Rc;
use std::collections::hash_map::HashMap;

use glium::glutin::{
    self,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
    window::WindowId,
};

use crate::window::Window;

pub struct Application {
    pub event_loop: glutin::event_loop::EventLoop<()>,
    windows: HashMap<WindowId, Rc<Window>>,
}

impl Application {
    pub fn new() -> Application {
        Application {
            event_loop: glutin::event_loop::EventLoop::<()>::new(),
            windows: HashMap::new(),
        }
    }

    pub fn register_window(&mut self, window: Rc<Window>) {
        self.windows.insert(window.get_id(), window);
    }

    pub fn start_event_loop(self) -> ! {
        let windows = self.windows;
        let mut close_requested = false;
        self.event_loop.run(move |event, _event_loop, control_flow| match event {
            Event::WindowEvent { event, window_id } => match event {
                WindowEvent::CloseRequested => {
                    close_requested = true;
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => {
                    close_requested = true;
                }
                WindowEvent::Resized { .. } => {
                    windows.get(&window_id).unwrap().request_redraw();
                }
                _ => {
                    windows.get(&window_id).unwrap().process_event(event);
                }
            },
            Event::MainEventsCleared => {
                if !close_requested {
                    for (_, window) in windows.iter() {
                        if window.redraw_needed() {
                            window.request_redraw();
                        }
                    }
                }
                if close_requested {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::RedrawRequested(window_id) => {
                windows.get(&window_id).unwrap().redraw();
            }
            Event::RedrawEventsCleared => {
                if close_requested {
                    *control_flow = ControlFlow::Exit;
                } else {
                    *control_flow = ControlFlow::Wait;
                }
            }
            _ => (),
        });
    }
}
