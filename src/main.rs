#![windows_subsystem = "windows"]

extern crate cgmath;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate glium;
extern crate backtrace;
extern crate image;
extern crate serde;
extern crate sys_info;
#[macro_use]
extern crate serde_derive;
extern crate rmp_serde;
extern crate rand;
extern crate alphanumeric_sort;

use std::cell::RefCell;
use std::env;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread;
use std::time::Duration;

use glium::glutin::{VirtualKeyCode, WindowEvent};
use glium::{glutin, Surface};

mod handle_panic;
mod image_cache;
mod shaders;
mod ui;

mod picture_panel;
use picture_panel::PicturePanel;

mod bottom_panel;
use bottom_panel::BottomPanel;

mod playback_manager;
use playback_manager::{LoadRequest, PlaybackManager};

mod window;
use window::*;

mod configuration;
use configuration::Configuration;

mod util;

// ========================================================
// Glorious main function
// ========================================================
fn main() {
    use std::boxed::Box;
    use std::panic;

    panic::set_hook(Box::new(handle_panic::handle_panic));

    Program::start();
}
// ========================================================

trait OptionRefClone {
    fn ref_clone(&self) -> Self;
}

impl OptionRefClone for Option<Rc<glium::texture::SrgbTexture2d>> {
    fn ref_clone(&self) -> Option<Rc<glium::texture::SrgbTexture2d>> {
        match *self {
            Some(ref image) => Some(image.clone()),
            None => None,
        }
    }
}

struct Program<'a> {
    configuration: &'a RefCell<Configuration>,
    config_file_path: PathBuf,

    window: &'a mut Window,
    picture_panel: &'a RefCell<PicturePanel>,
    playback_manager: &'a RefCell<PlaybackManager>,
    bottom_panel: BottomPanel<'a>,
}

impl<'a> Program<'a> {
    fn get_bg_color(light_theme: bool) -> [f32; 4] {
        if light_theme {
            [0.9, 0.9, 0.9, 0.0]
        } else {
            [0.02, 0.02, 0.02, 0.0]
        }
    }

    fn draw_picture(window: &mut Window, picture_panel: &mut PicturePanel, config: &Configuration) {
        let mut target = window.display().draw();

        let bg_color = Self::get_bg_color(config.light_theme);
        target.clear_color(bg_color[0], bg_color[1], bg_color[2], bg_color[3]);
        picture_panel.draw(&mut target, window, config);
        target.finish().unwrap();
    }

    fn start() {
        // Load config file
        let config_file_name = "cfg.bin";
        let exe_path = env::current_exe().unwrap();
        let exe_parent = exe_path.parent().unwrap();
        let config_file_path = exe_parent.join(config_file_name);
        let (config, first_run) =
            if let Ok(config) = Configuration::load(config_file_path.as_path()) {
                (RefCell::new(config), false)
            } else {
                (RefCell::new(Default::default()), true)
            };

        let mut events_loop = glutin::EventsLoop::new();
        let mut window = Window::new(&events_loop, &config.borrow());
        // Clear the screen right at the start so that the user sees the background color
        // whilst the image is loading.
        {
            let mut target = window.display().draw();
            let config = config.borrow();
            let bg_color = Self::get_bg_color(config.light_theme);
            target.clear_color(bg_color[0], bg_color[1], bg_color[2], bg_color[3]);
            target.finish().unwrap();
        }
        let mut picture_panel = PicturePanel::new(window.display(), BottomPanel::INITIAL_HEIGHT);
        picture_panel.set_show_usage(first_run);
        let playback_manager = RefCell::new(PlaybackManager::new());

        // Load image
        if let Some(img_path) = env::args().skip(1).next() {
            let img_path = PathBuf::from(img_path);
            let mut playback_manager = playback_manager.borrow_mut();
            playback_manager.request_load(LoadRequest::LoadSpecific(img_path));
            playback_manager.update_image(&mut window);
            picture_panel.set_image(playback_manager.image_texture().ref_clone());
        } else {
            window.set_title_filename("Drag and drop an image on the window.");
        }

        // Just quickly display the loaded image here before we load the remaining parts of the program
        Self::draw_picture(&mut window, &mut picture_panel, &config.borrow());

        let picture_panel = RefCell::new(picture_panel);
        let bottom_panel =
            BottomPanel::new(&mut window, &picture_panel, &playback_manager, &config);

        let mut program = Program {
            configuration: &config,
            config_file_path: config_file_path.clone(),
            window: &mut window,
            picture_panel: &picture_panel,
            playback_manager: &playback_manager,
            bottom_panel,
        };

        program.start_event_loop(&mut events_loop);

        let _ = program.configuration.borrow().save(config_file_path);
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
                                        if self.window.fullscreen() {
                                            self.picture_panel.borrow_mut().toggle_fullscreen(
                                                &mut self.window,
                                                &mut self.bottom_panel,
                                            );
                                        } else {
                                            running = false;
                                        }
                                    }
                                }
                            }
                        }
                        WindowEvent::Moved(position) => {
                            let mut config = self.configuration.borrow_mut();
                            config.window_x = position.x as i32;
                            config.window_y = position.y as i32;
                            // Don't you dare saving to file here.
                        }
                        WindowEvent::Resized(size) => {
                            let mut config = self.configuration.borrow_mut();
                            config.window_width = size.width as u32;
                            config.window_height = size.height as u32;
                        }
                        WindowEvent::Focused(false) => {
                            let config = self.configuration.borrow();
                            let _ = config.save(self.config_file_path.as_path());
                        }
                        _ => (),
                    }
                }

                // Pre events
                self.picture_panel.borrow_mut().pre_events();

                // Dispatch event
                self.bottom_panel.handle_event(&event, &self.window);
                // Playback manager is borrowed only after the bottom panel button callbacks
                // are finished
                let mut playback_manager = self.playback_manager.borrow_mut();
                self.picture_panel.borrow_mut().handle_event(
                    &event,
                    &mut self.window,
                    &mut self.bottom_panel,
                    &mut playback_manager,
                );

                // Update screen after a resize event or refresh
                if let Event::WindowEvent { event, .. } = event {
                    match event {
                        WindowEvent::Refresh => {
                            self.draw(&playback_manager, &mut self.picture_panel.borrow_mut())
                        }
                        _ => (),
                    }
                }
            });

            let mut playback_manager = self.playback_manager.borrow_mut();
            let mut picture_panel = self.picture_panel.borrow_mut();
            let load_requested = *playback_manager.load_request() != LoadRequest::None;
            playback_manager.update_image(&mut self.window);
            picture_panel.set_image(playback_manager.image_texture().ref_clone());

            self.draw(&playback_manager, &mut picture_panel);

            // Update dirctory after draw
            if load_requested {
                if let Err(err) = playback_manager.update_directory() {
                    eprintln!("{}", err);
                }
            }

            let should_sleep = {
                playback_manager.should_sleep() && picture_panel.should_sleep() && !load_requested
            };

            // Let other processes run for a bit.
            //thread::yield_now();
            if should_sleep {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn draw(&mut self, playback_manager: &PlaybackManager, picture_panel: &mut PicturePanel) {
        match self.window.display().gl_window().get_inner_size() {
            Some(window_size) => {
                if window_size.width <= 0.0 || window_size.height <= 0.0 {
                    return;
                }
            }
            None => return,
        }

        let mut target = self.window.display().draw();

        let config = self.configuration.borrow();
        let bg_color = Self::get_bg_color(config.light_theme);
        target.clear_color(bg_color[0], bg_color[1], bg_color[2], bg_color[3]);

        picture_panel.draw(&mut target, &self.window, &config);
        self.bottom_panel
            .draw(&mut target, playback_manager, &config);

        target.finish().unwrap();
    }
}
