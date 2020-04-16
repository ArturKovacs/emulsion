use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use cgmath::{Matrix4, Vector3};
use glium::glutin::event::{ElementState, MouseButton};
use glium::{uniform, Frame, Surface};

use crate::add_common_widget_functions;
use crate::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use crate::{DrawContext, Event, EventKind, Widget, WidgetData, WidgetError};
use crate::picture::Picture;
use crate::NextUpdate;

struct ButtonData {
    placement: WidgetPlacement,
    drawn_bounds: LogicalRect,
    visible: bool,

    click: bool,
    hover: bool,
    icon: Option<Rc<Picture>>,
    on_click: Option<Rc<dyn Fn()>>,

    rendered_valid: bool,
}
impl WidgetData for ButtonData {
    fn placement(&mut self) -> &mut WidgetPlacement {
        &mut self.placement
    }
    fn drawn_bounds(&mut self) -> &mut LogicalRect {
        &mut self.drawn_bounds
    }
    fn visible(&mut self) -> &mut bool {
        &mut self.visible
    }
}

pub struct Button {
    data: RefCell<ButtonData>,
}

impl Button {
    pub fn new() -> Button {
        Button {
            data: RefCell::new(ButtonData {
                placement: Default::default(),
                drawn_bounds: Default::default(),
                visible: true,
                click: false,
                hover: false,
                on_click: None,
                icon: None,
                rendered_valid: false,
            }),
        }
    }

    add_common_widget_functions!(data);

    /// Feel free to use `RefCell`s within the callback to satisfy the apparent constnes
    /// of the callback.
    pub fn set_on_click<T: Fn() + 'static>(&self, callback: T) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.on_click = Some(Rc::new(callback));
    }

    pub fn set_icon(&self, img: Option<Rc<Picture>>) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.icon = img;
        borrowed.rendered_valid = false;
    }
}

impl Widget for Button {
    fn is_valid(&self) -> bool {
        self.data.borrow().rendered_valid
    }

    fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<NextUpdate, WidgetError> {
        use glium::{Blend, BlendingFunction, LinearBlendingFactor};
        {
            let borrowed = self.data.borrow();

            let img_w = borrowed.drawn_bounds.size.vec.x;
            let img_h = borrowed.drawn_bounds.size.vec.y;

            // Model tranform
            let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
            let transform =
                Matrix4::from_translation(borrowed.drawn_bounds.pos.vec.extend(0.0)) * transform;
            // Projection
            let transform = context.projection_transform * transform;

            let image_draw_params = glium::DrawParameters {
                viewport: Some(*context.viewport),
                blend: Blend {
                    color: BlendingFunction::Addition {
                        source: LinearBlendingFactor::SourceAlpha,
                        destination: LinearBlendingFactor::OneMinusSourceAlpha,
                    },
                    ..Default::default()
                },
                ..Default::default()
            };
            let texture_size = [img_w, img_h];
            if let Some(ref icon) = borrowed.icon {
                let texture = icon.texture(context.display)?;
                let sampler = texture
                    .sampled()
                    .wrap_function(glium::uniforms::SamplerWrapFunction::Clamp)
                    .minify_filter(glium::uniforms::MinifySamplerFilter::Linear)
                    .magnify_filter(glium::uniforms::MagnifySamplerFilter::Linear);
                let uniforms = uniform! {
                    matrix: Into::<[[f32; 4]; 4]>::into(transform),
                    tex: sampler,
                    color: [1.0f32, 0.1, 0.5, 0.5],
                    texture_size: texture_size,
                    //brighten: if self.hover { 0.15f32 } else { 0.0f32 },
                    brighten: 0.0f32,
                    shadow_color: Into::<[f32; 3]>::into(Vector3::<f32>::new(0.0, 0.0, 0.0)),
                    shadow_offset: if borrowed.click {
                        0.5f32
                    } else {
                        if borrowed.hover { 0.7 } else { 1.0f32 }
                    }
                };
                target
                    .draw(
                        context.unit_quad_vertices,
                        context.unit_quad_indices,
                        context.textured_program,
                        &uniforms,
                        &image_draw_params,
                    )
                    .unwrap();
            } else {
                // building the uniforms
                let uniforms = uniform! {
                    matrix: Into::<[[f32; 4]; 4]>::into(transform),
                    color: [1.0f32, 0.1, 0.5, 0.5],
                    size: texture_size,
                    //brighten: if self.hover { 0.15f32 } else { 0.0f32 },
                    brighten: 0.0f32,
                    shadow_color: Into::<[f32; 3]>::into(Vector3::<f32>::new(0.0, 0.0, 0.0)),
                    shadow_offset: if borrowed.click {
                        0.5f32
                    } else {
                        if borrowed.hover { 0.7 } else { 1.0f32 }
                    }
                };
                target
                    .draw(
                        context.unit_quad_vertices,
                        context.unit_quad_indices,
                        context.colored_shadowed_program,
                        &uniforms,
                        &image_draw_params,
                    )
                    .unwrap();
            }
        }
        self.data.borrow_mut().rendered_valid = true;
        Ok(NextUpdate::Latest)
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
                    let on_click;
                    {
                        let mut borrowed = self.data.borrow_mut();
                        if borrowed.click && borrowed.hover {
                            on_click = borrowed.on_click.clone();
                        } else { on_click = None; }
                        borrowed.click = false;
                    }
                    if let Some(callback) = on_click {
                        callback();
                    }
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

    fn visible(&self) -> bool {
        self.data.borrow().visible
    }
}
