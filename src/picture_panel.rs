
use std::path::PathBuf;
use std::rc::Rc;
use std::mem;

use glium;
use glium::glutin::{VirtualKeyCode, WindowEvent};
use glium::glutin::dpi::LogicalSize;
use glium::index::PrimitiveType;

use glium::{Frame, Surface};
use glium::glutin;

use cgmath::ElementWise;
use cgmath::SquareMatrix;
use cgmath::{Matrix4, Vector2, Vector4};

use shaders;

use window::*;
use playback_manager::*;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

enum FileHoverState {
    Idle,
    HoveredFile{prev_file: PathBuf}
}

pub struct PicturePanel {
    vertex_buffer: glium::VertexBuffer<Vertex>,
    index_buffer: glium::IndexBuffer<u16>,
    program: glium::Program,
    image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
    zoom_scale: f32,
    cam_pos: Vector2<f32>,
    projection_transform: Matrix4<f32>,
    bottom: u32,

    // On Windows there is a bug that the cursor moved event will get
    // triggered with 0, 0 corrdinates when the window regains focus by
    // the user clicking into it.
    // To work around this we ignore the first mose move event after the window gains focus.
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
            zoom_scale: 1.0,
            cam_pos: Vector2::new(0.0, 0.0),
            projection_transform: Matrix4::identity(),
            bottom,

            file_hover_state: FileHoverState::Idle,

            ignore_one_mouse_move: false,

            last_mouse_pos: Vector2::new(0.0, 0.0),
            panning: false,

            should_sleep: true
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
        playback_manager: &mut PlaybackManager
    ) {
        if let glutin::Event::WindowEvent { event, .. } = event {
            let window_size = window.display().gl_window().get_inner_size().unwrap();
            let panel_size = self.get_panel_size(window_size);
            match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(keycode) = input.virtual_keycode {
                        if input.state == glutin::ElementState::Pressed {
                            match keycode {
                                VirtualKeyCode::Right | VirtualKeyCode::Left => {
                                    if keycode == VirtualKeyCode::Right {
                                        playback_manager.request_load(LoadRequest::LoadNext);
                                    } else {
                                        playback_manager.request_load(LoadRequest::LoadPrevious);
                                    }
                                }
                                VirtualKeyCode::Space => {
                                    if playback_manager.playback_state() == PlaybackState::Forward {
                                        playback_manager.pause_playback();
                                        let filename = playback_manager
                                            .current_filename().to_str().unwrap().to_owned();
                                        window.set_title_filename(filename.as_ref());
                                    } else {
                                        playback_manager.start_playback_forward();
                                        window.set_title_filename("PLAYING");
                                    };
                                }
                                VirtualKeyCode::R => {
                                    self.zoom_scale = 1.0;
                                    self.cam_pos = Vector2::new(0.0, 0.0);
                                }
                                _ => (),
                            }
                        } else {
                        }
                    }
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if *button == glutin::MouseButton::Left {
                        if *state == glutin::ElementState::Released {
                            self.panning = false;
                        } else {
                            let bottom_y = window_size.height as u32 - self.bottom;
                            if (self.last_mouse_pos.y as u32) < bottom_y {
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
                            let inv_projection_transform =
                                self.projection_transform.invert().unwrap();

                            let mut last_world_pos =
                                Self::get_mouse_proj(self.last_mouse_pos, window_size);
                            let mut curr_world_pos = Self::get_mouse_proj(pos_vec, window_size);

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

                            self.should_sleep = false;
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

                    let mut mouse_world = Self::get_mouse_proj(
                        self.last_mouse_pos,
                        panel_size,
                    );

                    let transformed = self.projection_transform.invert().unwrap()
                        * Vector4::new(mouse_world.x, mouse_world.y, 0.0, 1.0);
                    mouse_world.x = transformed.x;
                    mouse_world.y = transformed.y;

                    self.cam_pos += mouse_world * (1.0 - 1.0 / delta);
                    self.zoom_scale *= delta;

                    //println!("zoom_scale set to {}", self.zoom_scale);
                }
                WindowEvent::Focused(gained_focus) => {
                    if *gained_focus {
                        self.ignore_one_mouse_move = true;
                    }
                }
                WindowEvent::HoveredFile(file_name) => {
                    self.file_hover_state = FileHoverState::HoveredFile{prev_file: playback_manager.current_file_path()};
                    playback_manager.request_load(LoadRequest::LoadSpecific(file_name.clone()));
                }
                WindowEvent::HoveredFileCancelled => {
                    let mut tmp_hover_state = FileHoverState::Idle;
                    mem::swap(&mut self.file_hover_state, &mut tmp_hover_state);
                    if let FileHoverState::HoveredFile{prev_file} = tmp_hover_state {
                        playback_manager.request_load(LoadRequest::LoadSpecific(prev_file));
                    }
                }
                WindowEvent::DroppedFile(file_name) => {
                    match self.file_hover_state {
                        FileHoverState::Idle => {
                            playback_manager.request_load(LoadRequest::LoadSpecific(file_name.clone()));
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        }
    }


    pub fn draw(&mut self, target: &mut Frame, window: &Window) {
        let window_size = match window.display().gl_window().get_inner_size() {
            Some(size) => size,
            None => return
        };

        if window_size.width <= 0.0 || window_size.height <= 0.0 {
            return;
        }

        let panel_size = self.get_panel_size(window_size);

        self.update_projection_transform(panel_size);

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
            let sampler = if self.get_texel_size(panel_size) >= 6f32 {
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
                    bottom: self.bottom,
                    width: window_size.width as u32,
                    height: window_size.height as u32 - self.bottom
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
    }


    pub fn update_projection_transform(&mut self, panel_size: LogicalSize) {
        if let Some(ref texture) = self.image_texture {
            let img_w = texture.width() as f32;
            let img_h = texture.height() as f32;
            let img_aspect = img_w / img_h;
            // Projection tranform
            let window_aspect = panel_size.width as f32 / panel_size.height as f32;
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


    fn get_mouse_proj(mouse_screen: Vector2<f32>, panel_size: LogicalSize) -> Vector2<f32> {
        // Calculate mouse pos in "world space"
        //let window_size = self.display.gl_window().get_inner_size().unwrap();
        let panel_center =
            Vector2::new(panel_size.width as f32 * 0.5, panel_size.height as f32 * 0.5);
        let mut mouse_world = mouse_screen - panel_center;
        mouse_world.y *= -1.0;
        mouse_world.div_assign_element_wise(Vector2::new(
            panel_size.width as f32 * 0.5,
            panel_size.height as f32 * 0.5,
        ));
        mouse_world
    }

    fn get_texel_size(&self, panel_size: LogicalSize) -> f32 {
        if let Some(ref image_texture) = self.image_texture {
            (panel_size.width.min(panel_size.height) as f32
                / image_texture.width().max(image_texture.height()) as f32)
                * self.zoom_scale
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
}
