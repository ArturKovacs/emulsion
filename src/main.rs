#![windows_subsystem = "windows"]

extern crate cgmath;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate glium;
extern crate image;
extern crate sys_info;
extern crate backtrace;

use std::env;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use glium::glutin::{VirtualKeyCode, WindowEvent};
use glium::{glutin, Surface};
use glium::glutin::dpi::LogicalSize;
use glium::texture::{RawImage2d, SrgbTexture2d};

use glium::glutin::EventsLoop;

use cgmath::ElementWise;
use cgmath::SquareMatrix;
use cgmath::{Matrix4, Vector2, Vector4};

mod image_cache;
use image_cache::ImageCache;

mod handle_panic;
mod ui;
mod shaders;

mod picture_controller;
use picture_controller::PictureController;

mod window;
use window::*;

fn load_texture_without_cache(
    display: &glium::Display,
    image_path: &Path,
) -> SrgbTexture2d {
    let image = image::open(image_path).unwrap().to_rgba();

    texture_from_image(display, image)
}

fn texture_from_image(
    display: &glium::Display,
    image: image::RgbaImage,
) -> SrgbTexture2d {
    let image_dimensions = image.dimensions();
    let image = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);

    SrgbTexture2d::with_mipmaps(
        display,
        image,
        glium::texture::MipmapsOption::NoMipmap,
    ).unwrap()
}

struct Program {
    window: Window,
    ui: ui::Ui,
    picture_controller: PictureController,
}

impl Program {
    fn start() {
        let mut events_loop = glutin::EventsLoop::new();
        let mut window = Window::init(&events_loop);

        // TODO INITIALIZE THE UI AFTER THE IMAGE IS VISIBLE ON THE SCREEN.
        let mut ui = ui::Ui::new(&window.display, Window::BOTTOM_PANEL_HEIGHT);
        let exe_parent = std::env::current_exe().unwrap().parent().unwrap().to_owned();

        let button_texture = Rc::new(
            load_texture_without_cache(
                &window.display,
                &exe_parent.join("cogs.png")
            )
        );

        let button = ui.create_button(button_texture, ||());
        
        if let Some(button) = ui.get_button_mut(button) {
            button.set_callback(Box::new(|| println!("Clicked!")));
        }

        let mut picture_controller = PictureController::new(&window.display);

        let mut program = Program {
            window,
            ui,
            picture_controller
        };

        program.start_event_loop(&mut events_loop);
    }

    fn start_event_loop(&mut self, events_loop: &mut glutin::EventsLoop) {
        let mut running = true;
        // the main loop
        while running {
            events_loop.poll_events(|event| {
                use glutin::Event;
                if let Event::WindowEvent { ref event, .. } = event {
                    match event {
                        // Break from the main loop when the window is closed.
                        WindowEvent::CloseRequested => running = false,
                        WindowEvent::KeyboardInput { input, .. } => {
                            if let Some(keycode) = input.virtual_keycode {
                                if input.state == glutin::ElementState::Pressed {
                                    if keycode == VirtualKeyCode::Escape {
                                        running = false
                                    }
                                }
                            }
                        }
                        _ => (),
                    }
                }

                // Dispatch event
                self.picture_controller.handle_event(&event, &mut self.window);
                if let Event::WindowEvent { ref event, .. } = event {
                    let window_size = self.window.display.gl_window().get_inner_size().unwrap();
                    self.ui.window_event(&event, window_size);
                }

                // Update screen after a resize event or refresh
                if let Event::WindowEvent { event, .. } = event {
                    match event {
                        WindowEvent::Resized(..) | WindowEvent::Refresh => self.draw(),
                        _ => (),
                    }
                }
            });

            self.window.update_playback();

            if self.window.load_request != LoadRequest::None {
                let image = match self.window.image_texture {
                    Some(ref image) => Some(image.clone()),
                    None => None,
                };
                self.picture_controller.set_image(image);
            }

            self.draw();

            // Update dirctory only after draw
            if self.window.load_request != LoadRequest::None {
                self.window.image_cache.update_directory().unwrap();
            }

            let should_sleep = self.window.should_sleep() && self.picture_controller.should_sleep();

            // Let other processes run for a bit.
            //thread::yield_now();
            if should_sleep {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn draw(&mut self) {
        let mut target = self.window.display.draw();

        target.clear_color(0.9, 0.9, 0.9, 0.0);

        self.picture_controller.draw(&mut target, &self.window);
        self.ui.draw(&mut target);

        target.finish().unwrap();
    }
}

fn main() {
    use std::panic;
    use std::boxed::Box;

    panic::set_hook(Box::new(handle_panic::handle_panic));

    Program::start();
}
