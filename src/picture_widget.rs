
use std::cell::RefCell;
use std::rc::Rc;

use crate::shaders;

use gelatin::cgmath::{Matrix4, Vector3};
use gelatin::glium::glutin::event::{ElementState, MouseButton};
use gelatin::glium::{Display, Program, program, uniform, Frame, Surface, texture::SrgbTexture2d};

use gelatin::add_common_widget_functions;
use gelatin::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use gelatin::{DrawContext, Event, EventKind, Widget, WidgetData};

use std::time::{Duration, Instant};

struct PictureWidgetData {
    pub placement: WidgetPlacement,
    pub drawn_bounds: LogicalRect,

    pub click: bool,
    pub hover: bool,
    pub image_texture: Option<Rc<SrgbTexture2d>>,

    program: Program,
    img_texel_size: f32,
    image_fit: bool,
    img_pos: LogicalVector,

    last_click_time: Instant,
    last_mouse_pos: LogicalVector,
    panning: bool,
    moving_window: bool,

    pub rendered_valid: bool,
}
impl WidgetData for PictureWidgetData {
    fn placement(&mut self) -> &mut WidgetPlacement {
        &mut self.placement
    }
    fn drawn_bounds(&mut self) -> &mut LogicalRect {
        &mut self.drawn_bounds
    }
}
impl PictureWidgetData {
    fn fit_image_to_panel(&mut self) {
        let size = self.drawn_bounds.size.vec;
        let img_texel_size = if let Some(ref texture) = self.image_texture {
            let panel_aspect = size.x / size.y;
            let img_aspect = texture.width() as f32 / texture.height() as f32;

            let texel_size_to_fit_width = size.x / texture.width() as f32;
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
            self.img_pos = LogicalVector::new(
                size.x as f32 * 0.5,
                size.y as f32 * 0.5,
            );
            self.img_texel_size = img_texel_size;
            self.image_fit = true;
        }
    }
}

pub struct PictureWidget {
    data: RefCell<PictureWidgetData>,
}

impl PictureWidget {
    pub fn new(display: &Display) -> PictureWidget {
        let program = program!(display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::FRAGMENT_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::FRAGMENT_110
            },
        )
        .unwrap();
        PictureWidget {
            data: RefCell::new(PictureWidgetData {
                placement: Default::default(),
                click: false,
                hover: false,
                image_texture: None,
                drawn_bounds: Default::default(),
                rendered_valid: false,

                program,
                img_texel_size: 0.0,
                image_fit: true,
                img_pos: Default::default(),
                last_click_time: Instant::now() - Duration::from_secs(10),
                last_mouse_pos: Default::default(),
                panning: false,
                moving_window: false,
            }),
        }
    }

    add_common_widget_functions!(data);
}

impl Widget for PictureWidget {
    fn is_valid(&self) -> bool {
        self.data.borrow().rendered_valid
    }

    fn draw(&self, target: &mut Frame, context: &DrawContext) {
        {
            let mut data = self.data.borrow_mut();
            if data.image_fit {
                data.fit_image_to_panel();
            }
        }
        {
            use gelatin::glium::{Blend, BlendingFunction, LinearBlendingFactor};
            let data = self.data.borrow();
    
            let size = data.drawn_bounds.size.vec;
            let projection_transform = gelatin::cgmath::ortho(0.0, size.x, size.y, 0.0, -1.0, 1.0);
    
            let image_draw_params = gelatin::glium::DrawParameters {
                viewport: Some(context.logical_rect_to_viewport(&data.drawn_bounds)),
                ..Default::default()
            };
    
            if let Some(ref texture) = data.image_texture {
                let img_w = texture.width() as f32;
                let img_h = texture.height() as f32;
    
                let img_height_over_width = img_h / img_w;
                let image_display_width = data.img_texel_size * img_w;
    
                // Model tranform
                let image_display_height = image_display_width * img_height_over_width;
                let corner_x = (data.img_pos.vec.x - image_display_width * 0.5).floor();
                let corner_y = (data.img_pos.vec.y - image_display_height * 0.5).floor();
                let transform =
                    Matrix4::from_nonuniform_scale(image_display_width, image_display_height, 1.0);
                let transform =
                    Matrix4::from_translation(Vector3::new(corner_x, corner_y, 0.0)) * transform;
                // Projection tranform
                let transform = projection_transform * transform;
    
                let sampler = texture
                    .sampled()
                    .wrap_function(gelatin::glium::uniforms::SamplerWrapFunction::Clamp);
                let sampler = if data.img_texel_size >= 4f32 {
                    sampler.magnify_filter(gelatin::glium::uniforms::MagnifySamplerFilter::Nearest)
                } else {
                    sampler.magnify_filter(gelatin::glium::uniforms::MagnifySamplerFilter::Linear)
                };
                // building the uniforms
                let light_theme = true;
                let uniforms = uniform! {
                    matrix: Into::<[[f32; 4]; 4]>::into(transform),
                    bright_shade: if light_theme { 0.95f32 } else { 0.3f32 },
                    tex: sampler
                };
                target
                    .draw(
                        context.unit_quad_vertices,
                        context.unit_quad_indices,
                        &data.program,
                        &uniforms,
                        &image_draw_params,
                    )
                    .unwrap();
            }
        }
        self.data.borrow_mut().rendered_valid = true;
    }

    fn layout(&self, available_space: LogicalRect) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.default_layout(available_space);
    }

    fn handle_event(&self, event: &Event) {
        match event.kind {
            EventKind::MouseMove => {
                let mut borrowed = self.data.borrow_mut();
                borrowed.hover = borrowed.drawn_bounds.contains(event.cursor_pos);
            }
            EventKind::MouseButton { state, button: MouseButton::Left, .. } => match state {
                ElementState::Pressed => {
                    let mut borrowed = self.data.borrow_mut();
                    borrowed.click = borrowed.hover;
                }
                ElementState::Released => {
                    let mut borrowed = self.data.borrow_mut();
                    borrowed.click = false;
                }
            },
            _ => (),
        }
    }

    // No children for a button
    fn children(&self, _children: &mut Vec<Rc<dyn Widget>>) {}

    fn placement(&self) -> WidgetPlacement {
        self.data.borrow().placement
    }
}
