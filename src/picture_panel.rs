use std::mem;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, Duration};

use glium;
use glium::glutin::dpi::{LogicalSize, LogicalPosition};
use glium::glutin::{VirtualKeyCode, WindowEvent};
use glium::index::PrimitiveType;

use glium::glutin;
use glium::{Frame, Surface};

use cgmath;
use cgmath::SquareMatrix;
use cgmath::{Matrix4, Vector2, Vector3};

use shaders;

use configuration::Configuration;
use playback_manager::*;
use window::*;
use bottom_panel::BottomPanel;

use env;
use util::*;

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
    img_texel_size: f32,
    image_fit: bool,
    img_pos: Vector2<f32>,
    projection_transform: Matrix4<f32>,
    bottom: u32,
    panel_size: LogicalSize,

    // On Windows there is a bug that the cursor moved event will get
    // triggered with 0, 0 corrdinates when the window regains focus by
    // the user clicking into it.
    // To work around this we ignore the first mouse move event after the window gains focus.
    ignore_mouse_move_once: bool,

    only_update_mouse_pos_once: bool,

    last_click_time: SystemTime,
    last_mouse_pos: Vector2<f32>,
    panning: bool,
    moving_window: bool,

    file_hover_state: FileHoverState,

    usage_texture: glium::texture::SrgbTexture2d,
    show_usage: bool,
    color_program: glium::Program,

    should_sleep: bool,
}

impl PicturePanel {
    pub fn new(display: &glium::Display, bottom: u32) -> Self {
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

        let color_program = program!(display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::COLOR_F_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::COLOR_F_110
            },
        ).unwrap();

        let exe_parent = env::current_exe().unwrap().parent().unwrap().to_owned();
        let resource_dir = exe_parent.join("resource");

        let usage_texture =
            load_texture_without_cache(display, resource_dir.join("usage.png").as_ref());

        PicturePanel {
            vertex_buffer,
            index_buffer,
            program,
            image_texture: None,
            //img_display_width: 1,
            img_texel_size: 1.0,
            image_fit: true,
            img_pos: Vector2::new(0.0, 0.0),
            projection_transform: Matrix4::identity(),
            bottom,
            panel_size: LogicalSize {
                width: 1.0,
                height: 1.0,
            },

            file_hover_state: FileHoverState::Idle,

            usage_texture: usage_texture,
            show_usage: false,
            color_program: color_program,

            ignore_mouse_move_once: false,
            only_update_mouse_pos_once: false,

            last_click_time: SystemTime::UNIX_EPOCH,
            last_mouse_pos: Vector2::new(0.0, 0.0),
            panning: false,
            moving_window: false,

            should_sleep: true,
        }
    }

    pub fn set_bottom_pos(&mut self, bottom: u32) {
        self.bottom = bottom;
        let panel_size = self.panel_size;
        self.update_panel_size(panel_size);
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

    pub fn set_show_usage(&mut self, show_usage: bool) {
        self.show_usage = show_usage;
    }

    pub fn show_usage(&self) -> bool {
        self.show_usage
    }

    pub fn toggle_fullscreen(&mut self, window: &mut Window, bottom_panel: &mut BottomPanel) {
        let fullscreen = !window.fullscreen();
        window.set_fullscreen(fullscreen);
        self.only_update_mouse_pos_once = true;
        self.set_bottom_pos(if fullscreen { 0 } else { BottomPanel::INITIAL_HEIGHT });
        bottom_panel.set_enabled(!fullscreen);
    }

    pub fn handle_event(
        &mut self,
        event: &glutin::Event,
        window: &mut Window,
        bottom_panel: &mut BottomPanel,
        playback_manager: &mut PlaybackManager,
    ) {
        if let glutin::Event::WindowEvent { event, .. } = event {
            //let window_size = window.display().gl_window().get_inner_size().unwrap();
            //let panel_size = self.get_panel_size(window_size);
            match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if let Some(keycode) = input.virtual_keycode {
                        if keycode == VirtualKeyCode::Space {
                            self.panning = input.state == glutin::ElementState::Pressed;
                        }
                        if input.state == glutin::ElementState::Pressed {
                            match keycode {
                                VirtualKeyCode::Right | VirtualKeyCode::D => {
                                    playback_manager.request_load(LoadRequest::LoadNext);
                                }
                                VirtualKeyCode::Left => {
                                    playback_manager.request_load(LoadRequest::LoadPrevious);
                                }
                                VirtualKeyCode::A => {
                                    if input.modifiers.alt {
                                        self.toggle_playback(window, playback_manager);
                                    } else {
                                        playback_manager.request_load(LoadRequest::LoadPrevious);
                                    }
                                }
                                VirtualKeyCode::V => if input.modifiers.alt {
                                    self.toggle_playback(window, playback_manager);
                                }
                                VirtualKeyCode::P => if input.modifiers.ctrl {
                                    match playback_manager.playback_state() {
                                        PlaybackState::RandomPresent => {
                                            Self::pause_playback(window, playback_manager)
                                        }
                                        PlaybackState::Paused => {
                                            playback_manager.start_random_presentation();
                                            window.set_title_filename("Presenting In Random Order");
                                        }
                                        _ => (),
                                    }
                                } else {
                                    match playback_manager.playback_state() {
                                        PlaybackState::Present => {
                                            Self::pause_playback(window, playback_manager)
                                        }
                                        PlaybackState::Paused => {
                                            playback_manager.start_presentation();
                                            window.set_title_filename("Presenting");
                                        }
                                        _ => (),
                                    }
                                },
                                VirtualKeyCode::F => {
                                    self.fit_image_to_panel();
                                }
                                VirtualKeyCode::Q => {
                                    let panel_center = Vector2::new(
                                        self.panel_size.width as f32 * 0.5,
                                        self.panel_size.height as f32 * 0.5,
                                    );
                                    self.zoom_image(panel_center, 1.0);
                                    self.image_fit = false;
                                }
                                _ => (),
                            }
                        }
                    }
                }
                WindowEvent::Resized(new_window_size) => {
                    let new_panel_size = self.get_panel_size(*new_window_size);
                    self.update_panel_size(new_panel_size);
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    match *button {
                        glutin::MouseButton::Left => {
                            if *state == glutin::ElementState::Released {
                                self.moving_window = false;
                            } else {
                                if self.mouse_is_on_panel() {
                                    let now = SystemTime::now();
                                    let duration_since_last_click = 
                                        now.duration_since(self.last_click_time).unwrap();
                                    self.last_click_time = now;
                                    if duration_since_last_click < Duration::from_millis(250) {
                                        self.toggle_fullscreen(window, bottom_panel);
                                    } else {
                                        self.moving_window = true;
                                    }
                                }
                            }
                        }
                        glutin::MouseButton::Right => {
                            if *state == glutin::ElementState::Released {
                                self.panning = false;
                            } else {
                                if self.mouse_is_on_panel() { self.panning = true; }
                            }
                        }
                        _ => ()
                    }
                }
                WindowEvent::CursorMoved { position, .. } => if self.ignore_mouse_move_once {
                    self.ignore_mouse_move_once = false;
                } else {
                    // TODO: WRAP CURSOR AROUND WINDOW WHEN NEEDED
                    let pos_vec = Vector2::new(position.x as f32, position.y as f32);
                    if self.only_update_mouse_pos_once {
                        self.last_mouse_pos = pos_vec;
                        self.only_update_mouse_pos_once = false;
                    } else {
                        // Update transform
                        if self.panning {
                            let delta_pos = pos_vec - self.last_mouse_pos;
                            self.img_pos += delta_pos;
                            self.should_sleep = false;
                            self.image_fit = false;
                        }
                        if self.moving_window {
                            if window.fullscreen() {
                                // Disable fullscreen, teleport window
                                // so that the cursor position relative to the window
                                // remains the same proportionaly

                                let window_size =
                                    window.display().gl_window().get_inner_size().unwrap();
                                let cursor_screen_pos = self.last_mouse_pos;
                                let relative_pos_target = Vector2::new(
                                    cursor_screen_pos.x / window_size.width as f32,
                                    cursor_screen_pos.y / window_size.height as f32
                                );
                                self.toggle_fullscreen(window, bottom_panel);
                                let window_small_size = window.size_before_fullscreen();
                                let cursor_window_pos = Vector2::new(
                                    relative_pos_target.x * window_small_size.width as f32,
                                    relative_pos_target.y * window_small_size.height as f32
                                );
                                let window_pos = cursor_screen_pos - cursor_window_pos;
                                window.display().gl_window().set_position(LogicalPosition::new(
                                    window_pos.x as f64, window_pos.y as f64
                                ));
                            } else {
                                const BORDER: f32 = 80.0;
                                let window = window.display().gl_window();
                                let monitor = window.get_current_monitor();
                                let monitor_height = monitor.get_dimensions().height as f32;
                                let window_pos = window.get_position().unwrap();
                                let mut window_pos = Vector2::new(
                                    window_pos.x as f32, window_pos.y as f32
                                );
                                
                                let mut global_cursor_pos = pos_vec + window_pos;
                                global_cursor_pos.y =
                                    global_cursor_pos.y.max(BORDER).min(monitor_height - BORDER);
                                
                                let pos_vec = global_cursor_pos - window_pos;
                                let delta_pos = pos_vec - self.last_mouse_pos;
                                window_pos += delta_pos;
                                window.set_position(LogicalPosition::new(
                                    window_pos.x as f64, window_pos.y as f64
                                ));
                            }
                        } else {
                            self.last_mouse_pos = pos_vec;
                        }
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

                    let new_image_texel_size = (self.img_texel_size * delta).max(0.0);
                    let last_mouse_pos = self.last_mouse_pos;

                    self.zoom_image(last_mouse_pos, new_image_texel_size);
                    self.image_fit = false;
                }
                WindowEvent::Focused(gained_focus) => {
                    if *gained_focus {
                        self.ignore_mouse_move_once = true;
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

        if window_size.width <= 0.0 || window_size.height <= self.bottom as f64 {
            return;
        }

        self.update_projection_transform();

        if self.image_fit {
            self.fit_image_to_panel();
        }

        let image_draw_params = glium::DrawParameters {
            viewport: Some(glium::Rect {
                left: 0,
                bottom: self.bottom,
                width: window_size.width as u32,
                height: window_size.height as u32 - self.bottom,
            }),
            ..Default::default()
        };

        if let Some(ref texture) = self.image_texture {
            let img_w = texture.width() as f32;
            let img_h = texture.height() as f32;

            let img_height_over_width = img_h / img_w;
            let image_display_width = self.img_texel_size * img_w;

            // Model tranform
            let image_display_height = image_display_width * img_height_over_width;
            let corner_x = (self.img_pos.x - image_display_width * 0.5).floor();
            let corner_y = (self.img_pos.y - image_display_height * 0.5).floor();
            let transform =
                Matrix4::from_nonuniform_scale(image_display_width, image_display_height, 1.0);
            let transform =
                Matrix4::from_translation(Vector3::new(corner_x, corner_y, 0.0)) * transform;
            // Projection tranform
            let transform = self.projection_transform * transform;

            let sampler = texture
                .sampled()
                .wrap_function(glium::uniforms::SamplerWrapFunction::Clamp);
            let sampler = if self.img_texel_size >= 4f32 {
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

        if self.show_usage {
            let mut usage_bg_draw_params = image_draw_params.clone();
            usage_bg_draw_params.blend = glium::Blend {
                color: glium::BlendingFunction::Addition {
                    source: glium::LinearBlendingFactor::SourceAlpha,
                    destination: glium::LinearBlendingFactor::OneMinusSourceAlpha,
                },
                ..Default::default()
            };

            let transform = Matrix4::from_nonuniform_scale(2.0, 2.0, 1.0);
            let transform = Matrix4::from_translation(Vector3::new(-1.0, -1.0, 0.0)) * transform;

            let uniforms = uniform! {
                matrix: Into::<[[f32; 4]; 4]>::into(transform),
                color: [0.0, 0.0, 0.0, 0.75f32],
            };

            target
                .draw(
                    &self.vertex_buffer,
                    &self.index_buffer,
                    &self.color_program,
                    &uniforms,
                    &usage_bg_draw_params,
                )
                .unwrap();

            let sampler = self.usage_texture.sampled();

            let img_w = self.usage_texture.width() as f32;
            let img_h = self.usage_texture.height() as f32;

            // Model tranform
            let corner_x = (self.panel_size.width as f32 * 0.5 - img_w * 0.5).floor();
            let corner_y = (self.panel_size.height as f32 * 0.5 - img_h * 0.5).floor();
            let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
            let transform =
                Matrix4::from_translation(Vector3::new(corner_x, corner_y, 0.0)) * transform;
            let transform = cgmath::ortho(
                0.0,
                self.panel_size.width as f32,
                self.panel_size.height as f32,
                0.0,
                -1.0,
                1.0,
            ) * transform;

            let uniforms = uniform! {
                matrix: Into::<[[f32; 4]; 4]>::into(transform),
                bright_shade: if config.light_theme { 0.95f32 } else { 0.3f32 },
                tex: sampler
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
        self.projection_transform = cgmath::ortho(
            0.0,
            self.panel_size.width as f32,
            self.panel_size.height as f32,
            0.0,
            -1.0,
            1.0,
        );
    }

    fn update_panel_size(&mut self, new_panel_size: LogicalSize) {
        if self.image_fit {
            self.fit_image_to_panel();
        } else {
            let prev_panel_size = Vector2::new(
                self.panel_size.width as f32,
                self.panel_size.height as f32,
            );
            let new_panel_size =
                Vector2::new(new_panel_size.width as f32, new_panel_size.height as f32);
            let center_offset = (new_panel_size - prev_panel_size) * 0.5f32;
            self.img_pos += center_offset;
        }
        self.panel_size = new_panel_size;
    }

    fn get_panel_size(&self, window_size: LogicalSize) -> LogicalSize {
        LogicalSize {
            width: window_size.width,
            height: (window_size.height - self.bottom as f64).max(0.0),
        }
    }

    fn zoom_image(&mut self, anchor: Vector2<f32>, image_texel_size: f32) {
        self.img_pos = (image_texel_size / self.img_texel_size)
            * (self.img_pos - anchor) + anchor;
        self.img_texel_size = image_texel_size;
    }

    fn fit_image_to_panel(&mut self) {
        let img_texel_size = if let Some(ref texture) = self.image_texture {
            let panel_aspect = self.panel_size.width as f32 / self.panel_size.height as f32;
            let img_aspect = texture.width() as f32 / texture.height() as f32;

            let texel_size_to_fit_width = self.panel_size.width as f32 / texture.width() as f32;
            let img_texel_size = if img_aspect > panel_aspect {
                // The image is relatively wider than the panel
                texel_size_to_fit_width
            } else {
                texel_size_to_fit_width * (img_aspect / panel_aspect)
            };

            Some(img_texel_size)
        } else {
            None
        };

        if let Some(img_texel_size) = img_texel_size {
            self.img_pos = Vector2::new(
                self.panel_size.width as f32 * 0.5,
                self.panel_size.height as f32 * 0.5,
            );
            self.img_texel_size = img_texel_size;
            self.image_fit = true;
        }
    }

    fn pause_playback(window: &mut Window, playback_manager: &mut PlaybackManager) {
        playback_manager.pause_playback();
        let filename = playback_manager
            .current_filename()
            .to_str()
            .unwrap()
            .to_owned();
        window.set_title_filename(filename.as_ref());
    }

    fn mouse_is_on_panel(&self) -> bool {
        (self.last_mouse_pos.y as f64) < self.panel_size.height
    }

    fn toggle_playback(&mut self, window: &mut Window, playback_manager: &mut PlaybackManager) {
        match playback_manager.playback_state() {
            PlaybackState::Forward => {
                Self::pause_playback(window, playback_manager)
            }
            PlaybackState::Paused => {
                playback_manager.start_playback_forward();
                window.set_title_filename("Playing");
            }
            _ => (),
        }
    }
}
