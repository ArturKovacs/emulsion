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
    at_exit: Option<Box<dyn FnOnce()>>,
}

impl Application {
    pub fn new() -> Application {
        Application {
            event_loop: glutin::event_loop::EventLoop::<()>::new(),
            windows: HashMap::new(),
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

    pub fn start_event_loop(self) -> ! {
        let windows = self.windows;
        let mut at_exit = self.at_exit;
        let mut close_requested = false;
        let mut control_flow_source = *windows.keys().next().unwrap();
        self.event_loop.run(move |event, _event_loop, control_flow| {
            match event {
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
                    event @ _ => {
                        if let WindowEvent::Resized { .. } = event {
                            windows.get(&window_id).unwrap().request_redraw();
                        }
                        windows.get(&window_id).unwrap().process_event(event);
                    }
                },
                Event::MainEventsCleared => {
                    if !close_requested {
                        for (_, window) in windows.iter() {
                            if window.redraw_needed() {
                                window.request_redraw();
                                //event_loop
                            }
                        }
                    }
                    if close_requested {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                Event::RedrawRequested(window_id) => {
                    let new_control_flow = windows.get(&window_id).unwrap().redraw().into();
                    if control_flow_source == window_id {
                        *control_flow = new_control_flow;
                    } else if *control_flow != ControlFlow::Exit {
                        match new_control_flow {
                            ControlFlow::Exit => *control_flow = new_control_flow,
                            ControlFlow::Poll => {
                                *control_flow = new_control_flow;
                                control_flow_source = window_id;
                            }
                            ControlFlow::WaitUntil(new_time) => {
                                match *control_flow {
                                    ControlFlow::WaitUntil(orig_time) => {
                                        if new_time < orig_time {
                                            *control_flow = new_control_flow;
                                            control_flow_source = window_id;
                                        }
                                    }
                                    ControlFlow::Wait => {
                                        *control_flow = new_control_flow;
                                        control_flow_source = window_id;
                                    }
                                    _ => ()
                                }
                            }
                            _ => ()
                        }
                    }
                }
                Event::RedrawEventsCleared => {
                    if close_requested {
                        *control_flow = ControlFlow::Exit;
                    }
                }
                _ => (),
            }
            if *control_flow == ControlFlow::Exit {
                if let Some(at_exit) = at_exit.take() {
                    at_exit();
                }
            }
        });
    }
}
