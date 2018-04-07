#![windows_subsystem = "windows"]

extern crate cgmath;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate glium;
extern crate image;

use std::env;
use std::sync::Arc;

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
    display: glium::Display,

    image_cache: ImageCache,

    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
    image_texture: Option<Arc<glium::texture::SrgbTexture2d>>,
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

        let title = format!(
            "E M U L S I O N  ⬕  {}",
            if let Some(img_name) = img_name {
                img_name
            } else {
                ""
            }
        );

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

        let mut resulting_window = MainWindow {
            display,
            image_cache: ImageCache::new(5000),

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

        // the main loop
        events_loop.run_forever(|event| {
            match event {
                glutin::Event::WindowEvent { event, .. } => match event {
                    // Break from the main loop when the window is closed.
                    WindowEvent::Closed => return glutin::ControlFlow::Break,
                    WindowEvent::KeyboardInput { input, .. } => {
                        if input.virtual_keycode == Some(VirtualKeyCode::Escape) {
                            return glutin::ControlFlow::Break;
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        last_mouse_pos.x = position.0 as f32;
                        last_mouse_pos.y = position.1 as f32;
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        use glium::glutin::MouseScrollDelta;
                        let delta: f32 = match delta {
                            MouseScrollDelta::LineDelta(_, y) => {
                                println!("line");
                                y
                            }
                            MouseScrollDelta::PixelDelta(_, y) => {
                                println!("pixel");
                                y / 13.0
                            }
                        };
                        let delta = delta * 0.375;
                        let delta = if delta > 0.0 {
                            delta + 1.0
                        } else {
                            1.0 / (delta.abs() + 1.0)
                        };

                        // Calculate mouse pos in "world space"
                        let window_size = self.display.gl_window().get_inner_size().unwrap();
                        let window_center =
                            Vector2::new(window_size.0 as f32 * 0.5, window_size.1 as f32 * 0.5);
                        let mut mouse_world = last_mouse_pos - window_center;
                        mouse_world.y *= -1.0;
                        mouse_world.div_assign_element_wise(Vector2::new(
                            window_size.0 as f32 * 0.5,
                            window_size.1 as f32 * 0.5,
                        ));

                        let transformed = self.projection_transform.invert().unwrap()
                            * Vector4::new(mouse_world.x, mouse_world.y, 0.0, 1.0);
                        mouse_world.x = transformed.x;
                        mouse_world.y = transformed.y;

                        self.cam_pos += mouse_world * (1.0 - 1.0 / delta);
                        self.zoom_scale *= delta;

                        self.update_projection_transform();

                        //println!("zoom_scale set to {}", self.zoom_scale);
                        self.draw();
                    }
                    WindowEvent::Resized(..) => {
                        self.update_projection_transform();

                        self.draw()
                    }
                    _ => (),
                },
                _ => (),
            }
            glutin::ControlFlow::Continue
        });
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
}
