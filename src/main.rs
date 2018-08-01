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
use glium::index::PrimitiveType;
use glium::{glutin, Surface};
use glium::glutin::dpi::LogicalSize;
use glium::texture::{RawImage2d, SrgbTexture2d};

use cgmath::ElementWise;
use cgmath::SquareMatrix;
use cgmath::{Matrix4, Vector2, Vector4};

mod image_cache;
use image_cache::ImageCache;

mod handle_panic;
mod ui;
mod shaders;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

struct MainWindow {
    image_cache: ImageCache,

    display: glium::Display,

    ui: ui::Ui,

    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
    image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
    zoom_scale: f32,
    cam_pos: Vector2<f32>,
    projection_transform: Matrix4<f32>,
}

impl MainWindow {
    const BOTTOM_PANEL_HEIGHT: u32 = 32;

    fn init(events_loop: &glutin::EventsLoop) -> MainWindow {
        use glium::glutin::Icon;

        let img_path = env::args().skip(1).next();
        let img_name = match img_path {
            Some(ref img_path) => {
                let img_path = std::path::Path::new(img_path.as_str());
                img_path.file_name().unwrap().to_str()
            }
            _ => None,
        };

        let title = Self::create_title_filename(if let Some(name) = img_name { name } else { "" });

        let icon_name = "./emulsion32.png";
        let icon = Icon::from_path(icon_name)
            .unwrap_or_else(|_| panic!(format!("Could not load icon '{}'", icon_name)));

        let window = glutin::WindowBuilder::new()
            .with_title(title)
            .with_dimensions(LogicalSize::new(512.0, 512.0))
            .with_fullscreen(None)
            .with_window_icon(Some(icon))
            //.with_decorations(true)
            .with_visibility(true);

        //let context = glutin::ContextBuilder::new().with_gl(GlRequest::Specific(Api::OpenGl, (3, 1)));
        let context = glutin::ContextBuilder::new().with_gl_profile(glutin::GlProfile::Core);
        let display = glium::Display::new(window, context, events_loop).unwrap();

        // Clear the screen right at the start so that the user sees the background color
        // whilst the image is loading.
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
                vertex: shaders::VERTEX_140,
                fragment: shaders::FRAGMENT_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::FRAGMENT_110
            },
        ).unwrap();

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

        // TODO UI INITIALIZATION SHOLD BE MOVED AFTER THE IMAGE IS VISIBLE
        let ui = ui::Ui::new(&display, Self::BOTTOM_PANEL_HEIGHT);

        let mut resulting_window = MainWindow {
            image_cache: ImageCache::new(cache_capaxity, thread_count),
            display,
            ui,
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

        // set up ui elements here just for now
        let button_texture = Rc::new(
            Self::load_texture_without_cache(
                &resulting_window.display,
                Path::new("./cogs.png")
            )
        );

        let mut button = resulting_window.ui.create_button(button_texture, || println!("Clicked!"));

        resulting_window
    }

    fn start_event_loop(&mut self, events_loop: &mut glutin::EventsLoop) {
        let mut last_mouse_pos = Vector2::new(0.0, 0.0);
        let mut left_mouse_down = false;

        let get_mouse_proj = |mouse_screen: Vector2<f32>, window_size: LogicalSize| {
            // Calculate mouse pos in "world space"
            //let window_size = self.display.gl_window().get_inner_size().unwrap();
            let window_center =
                Vector2::new(window_size.width as f32 * 0.5, window_size.height as f32 * 0.5);
            let mut mouse_world = mouse_screen - window_center;
            mouse_world.y *= -1.0;
            mouse_world.div_assign_element_wise(Vector2::new(
                window_size.width as f32 * 0.5,
                window_size.height as f32 * 0.5,
            ));
            mouse_world
        };

        #[derive(PartialEq)]
        enum LoadRequest {
            None,
            LoadNext,
            LoadPrevious,
            LoadSpecific(PathBuf),
            Jump(i32),
        }

        #[derive(PartialEq)]
        enum PlaybackState {
            Paused,
            Forward,
            //Backward,
        }

        enum FileHoverState {
            Idle,
            HoveredFile{prev_file: PathBuf}
        }

        let mut playback_state = PlaybackState::Paused;
        let mut file_hover_state = FileHoverState::Idle;
        //let mut last_frame_time = Instant::now();
        let mut playback_start_time = Instant::now();
        let mut frame_count_since_playback_start = 0;

        // On Windows there is a bug that the cursor moved event will get
        // triggered with 0, 0 corrdinates when the window regains focus by
        // the user clicking into it.
        // To work around this we ignore the first mose move event after the window gains focus.
        let mut ignore_one_mouse_move = false;

        let framerate = 25.0;
        const NANOS_PER_SEC: u64 = 1000_000_000;
        let frame_delta_time_nanos = (NANOS_PER_SEC as f64 / framerate) as u64;

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
                        let window_size = self.display.gl_window().get_inner_size().unwrap();
                        self.ui.window_event(&event, window_size.height as u32);
                        match event {
                            // Break from the main loop when the window is closed.
                            WindowEvent::CloseRequested => running = false,
                            WindowEvent::KeyboardInput { input, .. } => {
                                if let Some(keycode) = input.virtual_keycode {
                                    if input.state == glutin::ElementState::Pressed {
                                        match keycode {
                                            VirtualKeyCode::Escape => running = false,
                                            VirtualKeyCode::Right | VirtualKeyCode::Left => {
                                                if keycode == VirtualKeyCode::Right {
                                                    load_request = LoadRequest::LoadNext;
                                                } else {
                                                    load_request = LoadRequest::LoadPrevious;
                                                }
                                            }
                                            VirtualKeyCode::Space => {
                                                playback_state =
                                                    if playback_state == PlaybackState::Forward {
                                                        let filename = self
                                                            .image_cache
                                                            .current_file_name().to_str().unwrap().to_owned();
                                                        self.set_title_filename(filename.as_ref());
                                                        PlaybackState::Paused
                                                    } else {
                                                        self.set_title_filename("PLAYING");
                                                        playback_start_time = Instant::now();
                                                        frame_count_since_playback_start = 0;
                                                        PlaybackState::Forward
                                                    };
                                            }
                                            VirtualKeyCode::R => {
                                                self.zoom_scale = 1.0;
                                                self.cam_pos = Vector2::new(0.0, 0.0);
                                                self.update_projection_transform();
                                                update_screen = true;
                                            }
                                            _ => (),
                                        }
                                    } else {
                                    }
                                }
                            }
                            WindowEvent::MouseInput { state, button, .. } => {
                                if button == glutin::MouseButton::Left {
                                    left_mouse_down = state == glutin::ElementState::Pressed;
                                }
                            }
                            WindowEvent::CursorMoved { position, .. } => {
                                if ignore_one_mouse_move {
                                    ignore_one_mouse_move = false;
                                } else {
                                    let pos_vec = Vector2::new(position.x as f32, position.y as f32);
                                    // Update transform
                                    if left_mouse_down {
                                        let inv_projection_transform =
                                            self.projection_transform.invert().unwrap();

                                        let mut last_world_pos =
                                            get_mouse_proj(last_mouse_pos, window_size);
                                        let mut curr_world_pos = get_mouse_proj(pos_vec, window_size);

                                        let tmp = inv_projection_transform
                                            * Vector4::new(
                                                last_world_pos.x,
                                                last_world_pos.y,
                                                0f32,
                                                1f32,
                                            );
                                        last_world_pos.x = tmp.x;
                                        last_world_pos.y = tmp.y;
                                        let tmp = inv_projection_transform
                                            * Vector4::new(
                                                curr_world_pos.x,
                                                curr_world_pos.y,
                                                0f32,
                                                1f32,
                                            );
                                        curr_world_pos.x = tmp.x;
                                        curr_world_pos.y = tmp.y;

                                        self.cam_pos += last_world_pos - curr_world_pos;

                                        self.update_projection_transform();
                                        update_screen = true;
                                        should_sleep = false;
                                    }

                                    last_mouse_pos = pos_vec;
                                }
                            }
                            WindowEvent::MouseWheel { delta, .. } => {
                                use glium::glutin::MouseScrollDelta;
                                let delta: f32 = match delta {
                                    MouseScrollDelta::LineDelta(_, y) => {
                                        //println!("line");
                                        y
                                    }
                                    MouseScrollDelta::PixelDelta(pos) => {
                                        //println!("pixel");
                                        (pos.y / 13.0) as f32
                                    }
                                };
                                let delta = delta * 0.375;
                                let delta = if delta > 0.0 {
                                    delta + 1.0
                                } else {
                                    1.0 / (delta.abs() + 1.0)
                                };

                                let mut mouse_world = get_mouse_proj(
                                    last_mouse_pos,
                                    self.display.gl_window().get_inner_size().unwrap(),
                                );

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
                            WindowEvent::Focused(gained_focus) => {
                                if gained_focus {
                                    ignore_one_mouse_move = true;
                                }
                                update_screen = true;
                            }
                            WindowEvent::Refresh => {
                                self.draw();
                            }
                            WindowEvent::HoveredFile(file_name) => {
                                file_hover_state = FileHoverState::HoveredFile{prev_file: self.image_cache.current_file_path()};
                                load_request = LoadRequest::LoadSpecific(file_name);
                            }
                            WindowEvent::HoveredFileCancelled => {
                                let mut tmp_hover_state = FileHoverState::Idle;
                                std::mem::swap(&mut file_hover_state, &mut tmp_hover_state);
                                if let FileHoverState::HoveredFile{prev_file} = tmp_hover_state {
                                    load_request = LoadRequest::LoadSpecific(prev_file);
                                }
                            }
                            WindowEvent::DroppedFile(file_name) => {
                                match file_hover_state {
                                    FileHoverState::Idle => {
                                        load_request = LoadRequest::LoadSpecific(file_name);
                                    }
                                    _ => (),
                                }
                            }
                            _ => (),
                        }
                    }
                    _ => (),
                }
            });

            if playback_state == PlaybackState::Paused {
                self.image_cache.process_prefetched(&self.display).unwrap();
                self.image_cache.send_load_requests();
            } else if load_request == LoadRequest::None {
                let elapsed = playback_start_time.elapsed();
                let elapsed_nanos =
                    elapsed.as_secs() * NANOS_PER_SEC + elapsed.subsec_nanos() as u64;
                let frame_step =
                    (elapsed_nanos / frame_delta_time_nanos) - frame_count_since_playback_start;
                if frame_step > 0 {
                    load_request = match playback_state {
                        PlaybackState::Forward => LoadRequest::Jump(frame_step as i32),
                        //PlaybackState::Backward => LoadRequest::Jump(-(frame_step as i32)),
                        PlaybackState::Paused => unreachable!(),
                    };
                    frame_count_since_playback_start += frame_step;
                } else {
                    self.image_cache.process_prefetched(&self.display).unwrap();

                    let nanos_since_last = elapsed_nanos % frame_delta_time_nanos;
                    const BUISY_WAIT_TRESHOLD: f32 = 0.8;
                    if nanos_since_last
                        > (frame_delta_time_nanos as f32 * BUISY_WAIT_TRESHOLD) as u64
                    {
                        // Just buisy wait if we are getting very close to the next frame swap
                        should_sleep = false;
                    } else {
                        self.image_cache.send_load_requests();
                    }
                }
            }

            //let should_sleep = load_request == LoadRequest::None && running && !update_screen;
            // Process long operations here
            let load_result = match load_request {
                LoadRequest::LoadNext => Some(self.image_cache.load_next(&self.display)),
                LoadRequest::LoadPrevious => Some(self.image_cache.load_prev(&self.display)),
                LoadRequest::LoadSpecific(ref file_path) => Some(
                    if let Some(file_name) = file_path.file_name() {
                        self.image_cache
                            .load_specific(&self.display, file_path.as_ref())
                            .map(|x| (x, OsString::from(file_name)))
                    } else {
                        Err(String::from("Could not extract filename").into())
                    }
                ),
                LoadRequest::Jump(jump_count) => {
                    Some(self.image_cache.load_jump(&self.display, jump_count))
                }
                LoadRequest::None => None,
            };
            if let Some(result) = load_result {
                match result {
                    Ok((texture, filename)) => {
                        self.image_texture = Some(texture);
                        // FIXME the program hangs when the title is set during a resize
                        // this is due to the way glutin/winit is architected.
                        // An issu already exists in winit proposing to redesign
                        // the even loop.
                        // Until that is implemented the title is simply not updated during
                        // playback.
                        if playback_state == PlaybackState::Paused {
                            self.set_title_filename(filename.to_str().unwrap());
                        }
                    }
                    Err(err) => {
                        self.image_texture = None;
                        self.set_title_filename("[none]");
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

                self.update_projection_transform();
                update_screen = true;
                should_sleep = false;
            }

            self.draw();

            if load_request != LoadRequest::None {
                self.image_cache.update_directory().unwrap();
            }

            // Let other processes run for a bit.
            //thread::yield_now();
            if should_sleep {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }

    fn update_projection_transform(&mut self) {
        if let Some(ref texture) = self.image_texture {
            let img_w = texture.width() as f32;
            let img_h = texture.height() as f32;
            let img_aspect = img_w / img_h;
            // Projection tranform
            let window_size = self.display.gl_window().get_inner_size().unwrap();
            let main_panel_size = self.get_main_panel_size(window_size);
            let window_aspect = main_panel_size.width as f32 / main_panel_size.height as f32;
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
        // Separator character used to be ⬕
        // But that one does not display correctly on Ubuntu 18.04

        format!("E M U L S I O N / {}", name)
    }

    fn load_image(&mut self, path: &Path) {
        self.image_texture = Some(self.image_cache.load_specific(&self.display, path).unwrap());
    }

    fn draw(&self) {
        let window = self.display.gl_window();
        let window_size = window.get_inner_size().unwrap();
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
            let image_draw_params = glium::DrawParameters {
                viewport: Some(glium::Rect {
                    left: 0,
                    bottom: Self::BOTTOM_PANEL_HEIGHT,
                    width: window_size.width as u32,
                    height: window_size.height as u32 - Self::BOTTOM_PANEL_HEIGHT
                }),
                .. Default::default()
            };
            target
                .draw(
                    &self.vertex_buffer,
                    &self.index_buffer,
                    &self.program,
                    &uniforms,
                    &image_draw_params,
                )
                .unwrap();
        }

        self.ui.draw(&mut target);

        target.finish().unwrap();
    }

    fn get_texel_size(&self) -> f32 {
        if let Some(ref image_texture) = self.image_texture {
            let window = self.display.gl_window();
            let window_size = window.get_inner_size().unwrap();

            let main_panel_size = self.get_main_panel_size(window_size);

            (main_panel_size.width.min(main_panel_size.height) as f32
                / image_texture.width().max(image_texture.height()) as f32)
                * self.zoom_scale
        } else {
            0f32
        }
    }

    fn get_main_panel_size(&self, window_size: LogicalSize) -> LogicalSize {
        LogicalSize {
            width: window_size.width,
            height: (window_size.height - Self::BOTTOM_PANEL_HEIGHT as f64).max(0.0),
        }
    }

    fn load_texture_without_cache(
        display: &glium::Display,
        image_path: &Path,
    ) -> SrgbTexture2d {
        let image = image::open(image_path).unwrap().to_rgba();

        Self::texture_from_image(display, image)
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
}


fn main() {
    use std::panic;
    use std::boxed::Box;

    panic::set_hook(Box::new(handle_panic::handle_panic));

    // I don't know how to Rust
    let mut events_loop = glutin::EventsLoop::new();
    let mut main_window = MainWindow::init(&events_loop);
    main_window.start_event_loop(&mut events_loop);

    // Just let the OS do the cleanup :D
    //std::mem::forget(main_window);
}
