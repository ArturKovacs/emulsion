use std::mem;
use std::path::PathBuf;
use std::rc::Rc;

use glium;
use glium::glutin::dpi::LogicalSize;
use glium::glutin::{VirtualKeyCode, WindowEvent};
use glium::index::PrimitiveType;

use glium::glutin;
use glium::{Frame, Surface};

use cgmath;
use cgmath::SquareMatrix;
use cgmath::{InnerSpace, Matrix, Matrix4, Vector2, Vector3, Vector4};

use shaders;

use configuration::Configuration;
use playback_manager::*;
use window::*;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

enum FileHoverState {
    Idle,
    HoveredFile { prev_file: PathBuf },
}

pub struct PicturePanel {
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
    image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
    img_display_width: u32,
    image_fit: bool,
    img_pos: Vector2<f32>,
    projection_transform: Matrix4<f32>,
    bottom: u32,
    panel_size: LogicalSize,

    // On Windows there is a bug that the cursor moved event will get
    // triggered with 0, 0 corrdinates when the window regains focus by
    // the user clicking into it.
    // To work around this we ignore the first mouse move event after the window gains focus.
    ignore_one_mouse_move: bool,

    last_mouse_pos: Vector2<f32>,
    panning: bool,

    file_hover_state: FileHoverState,

    should_sleep: bool,
}

impl PicturePanel {
    pub fn new(display: &glium::Display, bottom: u32) -> Self {
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
                display,
                &[
                    Vertex {
                        position: [0.0, 0.0],
                        tex_coords: [0.0, 0.0],
                    },
                    Vertex {
                        position: [0.0, 1.0],
                        tex_coords: [0.0, 1.0],
                    },
                    Vertex {
                        position: [1.0, 1.0],
                        tex_coords: [1.0, 1.0],
                    },
                    Vertex {
                        position: [1.0, 0.0],
                        tex_coords: [1.0, 0.0],
                    },
                ],
            ).unwrap()
        };

        // building the index buffer
        let index_buffer =
            glium::IndexBuffer::new(display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3])
                .unwrap();

        // compiling shaders and linking them together
        let program = program!(display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::FRAGMENT_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::FRAGMENT_110
            },
        ).unwrap();

        PicturePanel {
            vertex_buffer,
            index_buffer,
            program,
            image_texture: None,
            img_display_width: 1,
            image_fit: true,
            img_pos: Vector2::new(0.0, 0.0),
            projection_transform: Matrix4::identity(),
            bottom,
            panel_size: LogicalSize {width: 1.0, height: 1.0},

            file_hover_state: FileHoverState::Idle,

            ignore_one_mouse_move: false,

            last_mouse_pos: Vector2::new(0.0, 0.0),
            panning: false,

            should_sleep: true,
        }
    }

    pub fn pre_events(&mut self) {
        self.should_sleep = true;
    }

    pub fn should_sleep(&self) -> bool {
        self.should_sleep
    }

    pub fn set_image(&mut self, image_texture: Option<Rc<glium::texture::SrgbTexture2d>>) {
        self.image_texture = image_texture;
    }

    pub fn handle_event(
        &mut self,
        event: &glutin::Event,
        window: &mut Window,
        playback_manager: &mut PlaybackManager,
    ) {
        if let glutin::Event::WindowEvent { event, .. } = event {
            //let window_size = window.display().gl_window().get_inner_size().unwrap();
            //let panel_size = self.get_panel_size(window_size);
            match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(keycode) = input.virtual_keycode {
                        if input.state == glutin::ElementState::Pressed {
                            match keycode {
                                VirtualKeyCode::Right | VirtualKeyCode::D => {
                                    playback_manager.request_load(LoadRequest::LoadNext);
                                }
                                VirtualKeyCode::Left | VirtualKeyCode::A => {
                                    playback_manager.request_load(LoadRequest::LoadPrevious);
                                }
                                VirtualKeyCode::Space => {
                                    if playback_manager.playback_state() == PlaybackState::Forward {
                                        playback_manager.pause_playback();
                                        let filename = playback_manager
                                            .current_filename()
                                            .to_str()
                                            .unwrap()
                                            .to_owned();
                                        window.set_title_filename(filename.as_ref());
                                    } else {
                                        playback_manager.start_playback_forward();
                                        window.set_title_filename("PLAYING");
                                    };
                                }
                                VirtualKeyCode::F => {
                                    self.fit_image_to_panel();
                                }
                                VirtualKeyCode::Q => {
                                    let texture_width = if let Some(ref texture) = self.image_texture {
                                        Some(texture.width())
                                    } else {
                                        None
                                    };
                                    if let Some(texture_width) = texture_width {
                                        let panel_center = Vector2::new(
                                            self.panel_size.width as f32 * 0.5,
                                            self.panel_size.height as f32 * 0.5,
                                        );
                                        self.zoom_image(panel_center, texture_width);
                                        self.image_fit = false;
                                    }
                                }
                                _ => (),
                            }
                        } else {
                        }
                    }
                }
                WindowEvent::Resized(new_window_size) => {
                    let new_panel_size = self.get_panel_size(*new_window_size);
                    if self.image_fit {
                        self.fit_image_to_panel();
                    } else {
                        let prev_panel_size = Vector2::new(self.panel_size.width as f32, self.panel_size.height as f32);
                        let new_panel_size = Vector2::new(new_panel_size.width as f32, new_panel_size.height as f32);
                        let center_offset = (new_panel_size - prev_panel_size) * 0.5f32;
                        self.img_pos += center_offset;
                    }
                    self.panel_size = new_panel_size;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if *button == glutin::MouseButton::Left {
                        if *state == glutin::ElementState::Released {
                            self.panning = false;
                        } else {
                            if (self.last_mouse_pos.y as f64) < self.panel_size.height {
                                self.panning = true;
                            }
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    if self.ignore_one_mouse_move {
                        self.ignore_one_mouse_move = false;
                    } else {
                        let pos_vec = Vector2::new(position.x as f32, position.y as f32);
                        // Update transform
                        if self.panning {
                            self.img_pos += pos_vec - self.last_mouse_pos;
                            self.should_sleep = false;
                            self.image_fit = false;
                        }

                        self.last_mouse_pos = pos_vec;
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    use glium::glutin::MouseScrollDelta;
                    let delta: f32 = match delta {
                        MouseScrollDelta::LineDelta(_, y) => {
                            //println!("line");
                            *y
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

                    let new_image_display_width = (self.img_display_width as f32 * delta).max(1.0) as u32;
                    let last_mouse_pos = self.last_mouse_pos;

                    self.zoom_image(last_mouse_pos, new_image_display_width);
                    self.image_fit = false;
                }
                WindowEvent::Focused(gained_focus) => {
                    if *gained_focus {
                        self.ignore_one_mouse_move = true;
                    }
                }
                WindowEvent::HoveredFile(file_name) => {
                    self.file_hover_state = FileHoverState::HoveredFile {
                        prev_file: playback_manager.current_file_path(),
                    };
                    playback_manager.request_load(LoadRequest::LoadSpecific(file_name.clone()));
                }
                WindowEvent::HoveredFileCancelled => {
                    let mut tmp_hover_state = FileHoverState::Idle;
                    mem::swap(&mut self.file_hover_state, &mut tmp_hover_state);
                    if let FileHoverState::HoveredFile { prev_file } = tmp_hover_state {
                        playback_manager.request_load(LoadRequest::LoadSpecific(prev_file));
                    }
                }
                WindowEvent::DroppedFile(file_name) => match self.file_hover_state {
                    FileHoverState::Idle => {
                        playback_manager.request_load(LoadRequest::LoadSpecific(file_name.clone()));
                    }
                    _ => (),
                },
                _ => (),
            }
        }
    }

    pub fn draw(&mut self, target: &mut Frame, window: &Window, config: &Configuration) {
        let window_size = match window.display().gl_window().get_inner_size() {
            Some(size) => size,
            None => return,
        };

        if window_size.width <= 0.0 || window_size.height <= 0.0 {
            return;
        }

        let panel_size = self.get_panel_size(window_size);

        self.update_projection_transform();

        if self.image_fit {
            self.fit_image_to_panel();
        }

        if let Some(ref texture) = self.image_texture {
            let img_w = texture.width() as f32;
            let img_h = texture.height() as f32;

            let img_height_over_width = img_h / img_w;
            let image_display_width = self.img_display_width as f32;

            // Model tranform
            let image_display_height = image_display_width * img_height_over_width;
            let corner_x = (self.img_pos.x - image_display_width * 0.5).floor();
            let corner_y = (self.img_pos.y - image_display_height * 0.5).floor();
            let transform = Matrix4::from_nonuniform_scale(image_display_width, image_display_height, 1.0);
            let transform = Matrix4::from_translation(Vector3::new(corner_x, corner_y, 0.0)) * transform;
            // Projection tranform
            let transform = self.projection_transform * transform;

            let sampler = texture
                .sampled()
                .wrap_function(glium::uniforms::SamplerWrapFunction::Clamp);
            let sampler = if self.get_texel_size(panel_size) >= 6f32 {
                sampler.magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest)
            } else {
                sampler.magnify_filter(glium::uniforms::MagnifySamplerFilter::Linear)
            };
            // building the uniforms
            let uniforms = uniform! {
                matrix: Into::<[[f32; 4]; 4]>::into(transform),
                bright_shade: if config.light_theme { 0.95f32 } else { 0.3f32 },
                tex: sampler
            };
            let image_draw_params = glium::DrawParameters {
                viewport: Some(glium::Rect {
                    left: 0,
                    bottom: self.bottom,
                    width: window_size.width as u32,
                    height: window_size.height as u32 - self.bottom,
                }),
                ..Default::default()
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
    }

    fn update_projection_transform(&mut self) {
        self.projection_transform = cgmath::ortho(0.0, self.panel_size.width as f32, self.panel_size.height as f32, 0.0, -1.0, 1.0);
    }

    fn get_texel_size(&self, panel_size: LogicalSize) -> f32 {
        if let Some(ref image_texture) = self.image_texture {
            let img_w = image_texture.width() as f32;
            // The following line is mathematically equivalent to transforming the vector on the
            // right side by the matrix and then taking the X (first) component of the resulting
            // vector.
            let rendered_width_to_panel_width =
                self.projection_transform
                    .row(0)
                    .dot(Vector4::new(img_w * 0.5, 0.0, 0.0, 1.0));
            let rendered_width = rendered_width_to_panel_width * panel_size.width as f32;
            rendered_width / img_w
        //(panel_size.width.min(panel_size.height) as f32
        //    / image_texture.width().max(image_texture.height()) as f32)
        //    * self.zoom_scale
        } else {
            0f32
        }
    }

    fn get_panel_size(&self, window_size: LogicalSize) -> LogicalSize {
        LogicalSize {
            width: window_size.width,
            height: (window_size.height - self.bottom as f64).max(0.0),
        }
    }

    fn zoom_image(&mut self, anchor: Vector2<f32>, image_display_width: u32) {
        self.img_pos = (image_display_width as f32 / self.img_display_width as f32) * (self.img_pos-anchor) + anchor;
        self.img_display_width = image_display_width;
    }

    fn fit_image_to_panel(&mut self) {
        let img_display_width = if let Some(ref texture) = self.image_texture {
            let panel_aspect = self.panel_size.width as f32 / self.panel_size.height as f32;
            let img_aspect = texture.width() as f32 / texture.height() as f32;

            let img_display_width = if img_aspect > panel_aspect {
                // The image is relatively wider than the panel
                self.panel_size.width as u32
            } else {
                (self.panel_size.width as f32 * (img_aspect / panel_aspect)) as u32
            };

            Some(img_display_width)
        } else {
            None
        };
        
        if let Some(img_display_width) = img_display_width {
            self.img_pos = Vector2::new(self.panel_size.width as f32 * 0.5, self.panel_size.height as f32 * 0.5);
            self.img_display_width = img_display_width;
            self.image_fit = true;
        }
    }
}
