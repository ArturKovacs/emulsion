
use std::boxed::Box;

use glium;
use glium::{Surface, Frame};
use glium::glutin;

use cgmath::{Matrix4, Vector2, Vector3};

use ui::{ElementFunctions, DrawContext, Event};


pub struct Slider<'a> {
    callback: Box<Fn(u32, u32) -> () + 'a>,
    position: Vector2<f32>,
    size: Vector2<f32>,
    steps: u32,
    value: u32,
    hover: bool,
    click: bool,
}

impl<'a> Slider<'a> {
    const DISPLAY_OFFSET: f32 = 0.5;

    pub fn new(
        position: Vector2<f32>,
        size: Vector2<f32>,
        steps: u32,
        value: u32,
        callback: Box<Fn(u32, u32) -> () + 'a>,
    ) -> Self {
        Slider {
            callback,
            position,
            size,
            steps,
            value,
            hover: false,
            click: false,
        }
    }

    /// Sets the function that will be called on slider value change
    /// 
    /// # Arguments 
    /// * `callback` - The function that will be called. The first parameter of this function
    /// is the number of steps. The second parameter is the current value (step).
    pub fn set_callback(&mut self, callback: Box<Fn(u32, u32) -> () + 'a>) {
        self.callback = callback;
    }

    pub fn set_size(&mut self, size: Vector2<f32>) {
        self.size = size;
    }

    pub fn set_steps(&mut self, steps: u32, value: u32) {
        self.steps = steps;
        self.value = value;
    }

    pub fn value(&self) -> u32 {
        self.value
    }

    pub fn set_value(&mut self, value: u32) {
        self.value = value;
    }

    pub fn position(&self) -> Vector2<f32> {
        self.position
    }

    fn cursor_above(&self, cursor_position: &glutin::dpi::LogicalPosition) -> bool {
        let cursor_x = cursor_position.x as f32;
        let cursor_y = cursor_position.y as f32;

        let width = self.size.x;
        let height = self.size.y;

        cursor_x as f32 > self.position.x && cursor_x < (self.position.x + width)
            && cursor_y as f32 > self.position.y && cursor_y < (self.position.y + height)
    }

    fn value_from_cursor(&self, cursor_x: f32) -> u32 {
        let value_ratio = (cursor_x as f32 - self.position.x) / (self.size.x as f32);
        let value_almost = value_ratio * self.steps as f32 - Self::DISPLAY_OFFSET;

        value_almost.round().min(self.steps as f32 - 1f32).max(0f32) as u32
    }
}

impl<'a> ElementFunctions for Slider<'a> {
    fn draw(&self, target: &mut Frame, context: &DrawContext) {
        use glium::{Blend, BlendingFunction, LinearBlendingFactor};

        let width = self.size.x;
        let height = self.size.y;

        let image_draw_params = glium::DrawParameters {
            viewport: Some(*context.viewport),
            blend: Blend {
                color: BlendingFunction::Addition {
                    source: LinearBlendingFactor::SourceAlpha,
                    destination: LinearBlendingFactor::OneMinusSourceAlpha
                },
                .. Default::default()
            },
            .. Default::default()
        };

        // -----------------------
        // Draw vertical line at slider value
        // Do this first so the shadow we draw later will cover this line as well
        let value_ratio = (self.value as f32 + Self::DISPLAY_OFFSET) / (self.steps as f32);
        let slider_pos = Vector3::new(
            self.position.x + value_ratio * self.size.x,
            self.position.y,
            0.0
        );
        let color = [0.2, 0.2, 0.2, 1.0f32];

        let mut transform = Matrix4::from_nonuniform_scale(1.0, height, 1.0);
        transform = Matrix4::from_translation(slider_pos) * transform;
        transform = context.projection_transform * transform;
        let uniforms = uniform! {
            matrix: Into::<[[f32; 4]; 4]>::into(transform),
            color: color,
        };
        target.draw(
            context.unit_quad_vertices,
            context.unit_quad_indices,
            context.colored_program,
            &uniforms,
            &image_draw_params,
        ).unwrap();

        // -----------------------
        // Draw slider background (shadow)
        transform = Matrix4::from_nonuniform_scale(width, height, 1.0);
        transform = Matrix4::from_translation(self.position.extend(0.0)) * transform;
        transform = context.projection_transform * transform;

        // Transparent
        let color = [0.0, 0.0, 0.0, 0.0f32];

        let size = [width, height];
        let uniforms = uniform! {
            matrix: Into::<[[f32; 4]; 4]>::into(transform),
            color: color,
            size: size,
            shadow_offset: 0.8f32,
        };
        target.draw(
            context.unit_quad_vertices,
            context.unit_quad_indices,
            context.colored_shadowed_program,
            &uniforms,
            &image_draw_params,
        ).unwrap();
    }


    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseButton {button, state, position } => {
                if *button == glutin::MouseButton::Left {
                    if self.cursor_above(position) {
                        if *state == glutin::ElementState::Pressed {
                            self.click = true;
                            self.value = self.value_from_cursor(position.x as f32);
                            (self.callback)(self.steps, self.value);
                        } else {
                            self.click = false;
                        }
                    } else {
                        self.click = false;
                    }
                }
            }
            Event::MouseMove {position} => {
                self.hover = self.cursor_above(position);
                if self.click == true {
                    self.value = self.value_from_cursor(position.x as f32);
                    (self.callback)(self.steps, self.value);
                }
            }
        }
    }
}
