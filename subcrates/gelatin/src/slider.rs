use std::cell::RefCell;
use std::rc::Rc;

use cgmath::{Matrix4, Vector3};
use glium::glutin::event::{ElementState, MouseButton};
use glium::{uniform, Frame, Surface};

use crate::add_common_widget_functions;
use crate::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use crate::{DrawContext, Event, EventKind, Widget, WidgetData};

struct SliderData {
    pub placement: WidgetPlacement,
    pub drawn_bounds: LogicalRect,

    pub steps: u32,
    pub value: u32,
    pub click: bool,
    pub hover: bool,
    pub on_value_change: Option<Rc<dyn Fn()>>,

    pub rendered_valid: bool,
}
impl WidgetData for SliderData {
    fn placement(&mut self) -> &mut WidgetPlacement {
        &mut self.placement
    }
    fn drawn_bounds(&mut self) -> &mut LogicalRect {
        &mut self.drawn_bounds
    }
}

pub struct Slider {
    data: RefCell<SliderData>,
}

impl Slider {
    pub fn new() -> Slider {
        Slider {
            data: RefCell::new(SliderData {
                placement: Default::default(),
                steps: 1,
                value: 0,
                click: false,
                hover: false,
                on_value_change: None,
                drawn_bounds: Default::default(),
                rendered_valid: false,
            }),
        }
    }

    add_common_widget_functions!(data);

    pub fn steps(&self) -> u32 {
        self.data.borrow().steps
    }

    pub fn value(&self) -> u32 {
        self.data.borrow().value
    }

    pub fn set_steps(&self, steps: u32) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.steps = steps;
        borrowed.rendered_valid = false;
    }

    pub fn set_value(&self, value: u32) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.value = value;
        borrowed.rendered_valid = false;
    }

    /// Feel free to use `RefCell`s within the callback to satisfy the apparent constnes
    /// of the callback.
    pub fn set_on_value_change<T: Fn() + 'static>(&self, callback: T) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.on_value_change = Some(Rc::new(callback));
    }
}

impl Widget for Slider {
    fn is_valid(&self) -> bool {
        self.data.borrow().rendered_valid
    }

    fn draw(&self, target: &mut Frame, context: &DrawContext) {
        use glium::{Blend, BlendingFunction, LinearBlendingFactor};
        {
            let borrowed = self.data.borrow();

            let position = borrowed.drawn_bounds.pos.vec;
            let size = borrowed.drawn_bounds.size.vec;
            //let width = borrowed.drawn_bounds.size.vec.x;
            //let height = borrowed.drawn_bounds.size.vec.y;

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

            // -----------------------
            // Draw vertical line at slider value
            // Do this before the shadow so the shadow we draw later will cover this line as well
            let value_ratio = (borrowed.value as f32 + 0.5) / (borrowed.steps as f32);
            let slider_pos = Vector3::new(
                position.x + value_ratio * size.x,
                position.y,
                0.0,
            );
            let color = [0.25, 0.25, 0.25, 1.0f32];

            let mut transform = Matrix4::from_nonuniform_scale(1.0, size.y, 1.0);
            transform = Matrix4::from_translation(slider_pos) * transform;
            transform = context.projection_transform * transform;
            let uniforms = uniform! {
                matrix: Into::<[[f32; 4]; 4]>::into(transform),
                color: color,
            };
            target
                .draw(
                    context.unit_quad_vertices,
                    context.unit_quad_indices,
                    context.colored_program,
                    &uniforms,
                    &image_draw_params,
                )
                .unwrap();
            
            // -----------------------
            // Draw slider background (shadow)
            // Model tranform
            let transform = Matrix4::from_nonuniform_scale(size.x, size.y, 1.0);
            let transform =
                Matrix4::from_translation(borrowed.drawn_bounds.pos.vec.extend(0.0)) * transform;
            // Projection
            let transform = context.projection_transform * transform;
            
            // building the uniforms
            let uniforms = uniform! {
                matrix: Into::<[[f32; 4]; 4]>::into(transform),
                color: [0.0f32, 0.0, 0.0, 0.0],
                size: [size.x, size.y],
                brighten: 0.0f32,
                shadow_color: Into::<[f32; 3]>::into(Vector3::<f32>::new(0.0, 0.0, 0.0)),
                shadow_offset: 0.7f32,
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
        self.data.borrow_mut().rendered_valid = true;
    }

    fn layout(&self, available_space: LogicalRect) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.default_layout(available_space);
    }

    fn handle_event(&self, event: &Event) {
        match event.kind {
            EventKind::MouseMove => {
                // We jugle around the `on_value_change` callback so that when it gets called,
                // `self.data` is not borrowed.
                let on_value_change;
                {
                    let mut borrowed = self.data.borrow_mut();
                    borrowed.hover = borrowed.drawn_bounds.contains(event.cursor_pos);
                    if borrowed.click {
                        let prev_value = borrowed.value;
                        let relative_cursor_x = event.cursor_pos.vec.x - borrowed.drawn_bounds.pos.vec.x;
                        let proportion = (relative_cursor_x / borrowed.drawn_bounds.size.vec.x).max(0.0).min(1.0);
                        let stepsf = borrowed.steps as f32;
                        borrowed.value = 
                            (proportion * (1.0 + 1.0 / stepsf) * (stepsf - 1.0)).floor() as u32;
                        if borrowed.value != prev_value {
                            on_value_change = borrowed.on_value_change.clone();
                        } else { on_value_change = None; }
                    } else { on_value_change = None; }
                }
                if let Some(callback) = on_value_change { callback(); }
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
