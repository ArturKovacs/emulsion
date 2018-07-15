#![windows_subsystem = "windows"]

extern crate cgmath;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate glium;
extern crate image;
extern crate sys_info;

use std::env;
use std::rc::Rc;
use std::ffi::OsString;
use std::thread;
use std::time;
use std::io::Write;

use glium::{glutin, Surface};
use glium::index::PrimitiveType;
use glium::glutin::{VirtualKeyCode, WindowEvent};

use cgmath::{Matrix4, Vector2, Vector4};
use cgmath::SquareMatrix;
use cgmath::ElementWise;

mod image_cache;
use image_cache::ImageCache;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

struct MainWindow {
    image_cache: ImageCache,

    display: glium::Display,

    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
    image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
    zoom_scale: f32,
    cam_pos: Vector2<f32>,
    projection_transform: Matrix4<f32>,
}

impl MainWindow {
    fn init(events_loop: &glutin::EventsLoop) -> MainWindow {
        let img_path = env::args().skip(1).next();
        let img_name = match img_path {
            Some(ref img_path) => {
                let img_path = std::path::Path::new(img_path.as_str());
                img_path.file_name().unwrap().to_str()
            }
            _ => None,
        };

        let title = Self::create_title_filename(if let Some(name) = img_name { name } else { "" });

        let window = glutin::WindowBuilder::new()
            .with_title(title)
            .with_dimensions(512, 512)
            .with_fullscreen(None)
            //.with_decorations(true)
            .with_visibility(true);
        //let context = glutin::ContextBuilder::new().with_gl(GlRequest::Specific(Api::OpenGl, (3, 1)));
        let context = glutin::ContextBuilder::new().with_gl_profile(glutin::GlProfile::Core);
        let display = glium::Display::new(window, context, events_loop).unwrap();

        // Clear the screen right at the start so that the user sees a black window instead
        // of white while the image is loading.
        {
            let mut target = display.draw();
            target.clear_color(0.9, 0.9, 0.9, 0.0);
            target.finish().unwrap();
        }

        // building the vertex buffer, which contains all the vertices that we will draw
        let vertex_buffer = {
            glium::VertexBuffer::new(
                &display,
                &[
                    Vertex {
                        position: [-0.5, -0.5],
                        tex_coords: [0.0, 1.0],
                    },
                    Vertex {
                        position: [-0.5, 0.5],
                        tex_coords: [0.0, 0.0],
                    },
                    Vertex {
                        position: [0.5, 0.5],
                        tex_coords: [1.0, 0.0],
                    },
                    Vertex {
                        position: [0.5, -0.5],
                        tex_coords: [1.0, 1.0],
                    },
                ],
            ).unwrap()
        };

        // building the index buffer
        let index_buffer =
            glium::IndexBuffer::new(&display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3])
                .unwrap();

        // compiling shaders and linking them together
        let program = program!(&display,
            140 => {
                vertex: include_str!("shaders/vertex_140.glsl"),
                fragment: include_str!("shaders/fragment_140.glsl")
            },

            110 => {
                vertex: include_str!("shaders/vertex_110.glsl"),
                fragment: include_str!("shaders/fragment_110.glsl")
            },
        ).unwrap();

        let cache_capaxity = match sys_info::mem_info() {
            ::std::result::Result::Ok(value) => {
                // value originally reported in KiB
                ((value.total / 8) * 1024) as isize
            },
            _ => {
                println!("Could not get system memory size, using default value");
                // bytes
                500_000_000
            },
        };

        let mut resulting_window = MainWindow {
            image_cache: ImageCache::new(cache_capaxity),
            display,
            

            vertex_buffer,
            index_buffer,
            program,
            image_texture: None,
            zoom_scale: 1.0,
            cam_pos: Vector2::new(0.0, 0.0),
            projection_transform: Matrix4::identity(),
        };

        if let Some(img_path) = env::args().skip(1).next() {
            resulting_window.load_image(img_path.as_ref());
        };

        resulting_window
    }

    fn start_event_loop(&mut self, events_loop: &mut glutin::EventsLoop) {
        let mut last_mouse_pos = Vector2::new(0.0, 0.0);
        let mut left_mouse_down = false;

        let get_mouse_proj = |mouse_screen: Vector2<f32>, window_size: (u32, u32)| {
            // Calculate mouse pos in "world space"
            //let window_size = self.display.gl_window().get_inner_size().unwrap();
            let window_center = Vector2::new(
                window_size.0 as f32 * 0.5,
                window_size.1 as f32 * 0.5,
            );
            let mut mouse_world = mouse_screen - window_center;
            mouse_world.y *= -1.0;
            mouse_world.div_assign_element_wise(Vector2::new(
                window_size.0 as f32 * 0.5,
                window_size.1 as f32 * 0.5,
            ));
            mouse_world
        };

        #[derive(PartialEq)]
        enum LoadRequest {
            None,
            LoadNext,
            LoadPrevious,
            LoadSpecific(String),
        }
        
        // the main loop
        let mut running = true;
        while running {
            let mut load_request = LoadRequest::None;
            let mut update_screen = false;
            let mut should_sleep = true;
            events_loop.poll_events(|event| {
                match event {
                    glutin::Event::Awakened => {
                        update_screen = true;
                    }
                    glutin::Event::WindowEvent { event, .. } => {
                        //update_screen = true; // in case of any event at all, update the screen
                        match event {
                            // Break from the main loop when the window is closed.
                            WindowEvent::Closed => running = false,
                            WindowEvent::KeyboardInput { input, .. } => {
                                if input.state == glutin::ElementState::Pressed {
                                    if let Some(keycode) = input.virtual_keycode {
                                        match keycode {
                                            VirtualKeyCode::Escape => running = false,
                                            VirtualKeyCode::Right | VirtualKeyCode::Left => {
                                                if keycode == VirtualKeyCode::Right {
                                                    load_request = LoadRequest::LoadNext;
                                                } else {
                                                    load_request = LoadRequest::LoadPrevious;
                                                }
                                            }
                                            _ => (),
                                        }
                                    }
                                }
                            }
                            WindowEvent::MouseInput {state, button, ..} => {
                                if button == glutin::MouseButton::Left {
                                    left_mouse_down = state == glutin::ElementState::Pressed;
                                }
                            }
                            WindowEvent::CursorMoved { position, .. } => {
                                let pos_vec = Vector2::new(position.0 as f32, position.1 as f32);
                                // Update transform
                                if left_mouse_down {
                                    let inv_projection_transform = self.projection_transform.invert().unwrap();

                                    let window_size = self.display.gl_window().get_inner_size().unwrap();
                                    let mut last_world_pos = get_mouse_proj(last_mouse_pos, window_size);
                                    let mut curr_world_pos = get_mouse_proj(pos_vec, window_size);

                                    let tmp = inv_projection_transform * Vector4::new(last_world_pos.x, last_world_pos.y, 0f32, 1f32);
                                    last_world_pos.x = tmp.x;
                                    last_world_pos.y = tmp.y;
                                    let tmp = inv_projection_transform * Vector4::new(curr_world_pos.x, curr_world_pos.y, 0f32, 1f32);
                                    curr_world_pos.x = tmp.x;
                                    curr_world_pos.y = tmp.y;

                                    self.cam_pos += last_world_pos - curr_world_pos;

                                    self.update_projection_transform();
                                    update_screen = true;
                                    should_sleep = false;
                                }

                                last_mouse_pos = pos_vec;
                            }
                            WindowEvent::MouseWheel { delta, .. } => {
                                use glium::glutin::MouseScrollDelta;
                                let delta: f32 = match delta {
                                    MouseScrollDelta::LineDelta(_, y) => {
                                        //println!("line");
                                        y
                                    }
                                    MouseScrollDelta::PixelDelta(_, y) => {
                                        //println!("pixel");
                                        y / 13.0
                                    }
                                };
                                let delta = delta * 0.375;
                                let delta = if delta > 0.0 {
                                    delta + 1.0
                                } else {
                                    1.0 / (delta.abs() + 1.0)
                                };

                                let mut mouse_world = get_mouse_proj(last_mouse_pos, self.display.gl_window().get_inner_size().unwrap());

                                let transformed = self.projection_transform.invert().unwrap()
                                    * Vector4::new(mouse_world.x, mouse_world.y, 0.0, 1.0);
                                mouse_world.x = transformed.x;
                                mouse_world.y = transformed.y;

                                self.cam_pos += mouse_world * (1.0 - 1.0 / delta);
                                self.zoom_scale *= delta;

                                self.update_projection_transform();

                                //println!("zoom_scale set to {}", self.zoom_scale);
                                update_screen = true;
                            }
                            WindowEvent::Resized(..) => {
                                self.update_projection_transform();
                                self.draw(); // Update immediately on resize.
                            }
                            _ => (),
                        }
                    },
                    _ => (),
                }
            });
            //let should_sleep = load_request == LoadRequest::None && running && !update_screen;
            // Process long operations here
            let load_result = match load_request {
                LoadRequest::LoadNext => Some(self.image_cache.load_next(&self.display)),
                LoadRequest::LoadPrevious => Some(self.image_cache.load_prev(&self.display)),
                LoadRequest::LoadSpecific(filename) => Some(
                    self.image_cache
                        .load_specific(&self.display, filename.as_str())
                        .map(|x| (x, OsString::from(filename))),
                ),
                LoadRequest::None => None,
            };
            if let Some(result) = load_result {
                match result {
                    Ok((texture, filename)) => {
                        self.image_texture = Some(texture);
                        self.set_title_filename(filename.to_str().unwrap());
                    }
                    Err(err) => {
                        self.image_texture = None;
                        self.set_title_filename("[none]");
                        let stderr = &mut ::std::io::stderr();
                        let stderr_errmsg = "Error writing to stderr";
                        writeln!(stderr, "Error occured while loading image: {}", err).expect(stderr_errmsg);
                        for e in err.iter().skip(1) {
                            writeln!(stderr, "... caused by: {}", e).expect(stderr_errmsg);
                        }
                        if let Some(backtrace) = err.backtrace() {
                            writeln!(stderr, "backtrace: {:?}", backtrace).expect(stderr_errmsg);
                        }
                        writeln!(stderr).expect(stderr_errmsg);
                    }
                }
                
                self.update_projection_transform();
                update_screen = true;
                should_sleep = false;
            }

            if update_screen {
                self.draw();
            }
            
            // Let other processes run for a bit.
            //thread::yield_now();
            if should_sleep {
                thread::sleep(time::Duration::from_millis(1));
            }
        }
    }

    fn update_projection_transform(&mut self) {
        if let Some(ref texture) = self.image_texture {
            let img_w = texture.width() as f32;
            let img_h = texture.height() as f32;
            let img_aspect = img_w / img_h;
            // Projection tranform
            let (window_w, window_h) = self.display.gl_window().get_inner_size().unwrap();
            let window_aspect = window_w as f32 / window_h as f32;
            let (camera_width, camera_height) = if img_aspect < window_aspect {
                // Window is wider than image relatively
                (img_h * window_aspect, img_h)
            } else {
                // Window is taller than image relatively
                (img_w, img_w * (1.0 / window_aspect))
            };
            let cam_scale_x = self.zoom_scale / camera_width;
            let cam_scale_y = self.zoom_scale / camera_height;
            self.projection_transform =
                Matrix4::from_nonuniform_scale(cam_scale_x * 2.0, cam_scale_y * 2.0, 1.0);
        }
    }

    fn set_title_filename(&mut self, name: &str) {
        self.display
            .gl_window()
            .set_title(Self::create_title_filename(name).as_ref());
    }

    fn create_title_filename(name: &str) -> String {
        format!("E M U L S I O N  ⬕  {}", name)
    }

    fn load_image(&mut self, path: &str) {
        self.image_texture = Some(self.image_cache.load_specific(&self.display, path).unwrap());
    }

    fn draw(&self) {
        // drawing a frame
        let mut target = self.display.draw();
        target.clear_color(0.9, 0.9, 0.9, 0.0);
        if let Some(ref texture) = self.image_texture {
            let img_w = texture.width() as f32;
            let img_h = texture.height() as f32;

            // Model tranform
            let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
            // View transform
            let transform = Matrix4::from_translation(-1.0 * self.cam_pos.extend(0.0)) * transform;
            // Projection tranform
            let transform = self.projection_transform * transform;

            let sampler = texture
                .sampled()
                .wrap_function(glium::uniforms::SamplerWrapFunction::Clamp);
            let sampler = if self.get_texel_size() >= 6f32 {
                sampler.magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest)
            } else {
                sampler.magnify_filter(glium::uniforms::MagnifySamplerFilter::Linear)
            };
            // building the uniforms
            let uniforms = uniform! {
                matrix: Into::<[[f32; 4]; 4]>::into(transform),
                tex: sampler
            };
            target
                .draw(
                    &self.vertex_buffer,
                    &self.index_buffer,
                    &self.program,
                    &uniforms,
                    &Default::default(),
                )
                .unwrap();
        }
        target.finish().unwrap();
    }

    fn get_texel_size(&self) -> f32 {
        if let Some(ref image_texture) = self.image_texture {
            let window = self.display.gl_window();
            let (window_w, window_h) = window.get_inner_size().unwrap();

            (window_w.min(window_h) as f32
                / image_texture.width().max(image_texture.height()) as f32)
                * self.zoom_scale
        } else {
            0f32
        }
    }
}

fn main() {
    // I don't know how to Rust
    let mut events_loop = glutin::EventsLoop::new();
    let mut main_window = MainWindow::init(&events_loop);
    main_window.start_event_loop(&mut events_loop);

    // Just let the OS do the cleanup :D
    //std::mem::forget(main_window);
}
