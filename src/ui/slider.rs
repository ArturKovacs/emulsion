use std::rc::Rc;

use glium;
use glium::glutin;
use glium::{Frame, Surface};

use cgmath::{Matrix4, Vector2, Vector3};

use ui::{DrawContext, ElementFunctions, Event};

pub struct Slider<'callback_ref> {
    callback: Rc<Fn() + 'callback_ref>,
    position: Vector2<f32>,
    size: Vector2<f32>,
    shadow_color: Vector3<f32>,
    steps: u32,
    value: u32,
    hover: bool,
    click: bool,
    step_bg: Vec<bool>,
    step_bg_color: [f32; 4],
}

impl<'callback_ref> Slider<'callback_ref> {
    const DISPLAY_OFFSET: f32 = 0.5;

    pub fn new<F>(
        position: Vector2<f32>,
        size: Vector2<f32>,
        steps: u32,
        value: u32,
        callback: F,
    ) -> Self
    where
        F: Fn() + 'callback_ref,
    {
        Slider {
            callback: Rc::new(callback),
            position,
            size,
            shadow_color: Vector3::new(0.0, 0.0, 0f32),
            steps,
            value,
            hover: false,
            click: false,
            step_bg: Vec::new(),
            step_bg_color: [0.4, 0.4, 0.4, 1.0f32],
        }
    }

    /// Sets the function that will be called on slider value change
    ///
    /// # Arguments
    /// * `callback` - The function that will be called. The first parameter of this function
    /// is the number of steps. The second parameter is the current value (step).
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn() + 'callback_ref,
    {
        self.callback = Rc::new(callback);
    }

    pub fn set_size(&mut self, size: Vector2<f32>) {
        self.size = size;
    }

    pub fn set_steps(&mut self, steps: u32, value: u32) {
        self.steps = steps;
        self.value = value;
    }

    pub fn set_step_bg(&mut self, step_bg: Vec<bool>) {
        self.step_bg = step_bg;
    }

    pub fn set_step_bg_color(&mut self, color: [f32; 4]) {
        self.step_bg_color = color;
    }

    pub fn set_shadow_color(&mut self, color: Vector3<f32>) {
        self.shadow_color = color;
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

    pub fn set_position(&mut self, pos: Vector2<f32>) {
        self.position = pos;
    }

    fn cursor_above(&self, cursor_position: &glutin::dpi::LogicalPosition) -> bool {
        let cursor_x = cursor_position.x as f32;
        let cursor_y = cursor_position.y as f32;

        let width = self.size.x;
        let height = self.size.y;

        cursor_x as f32 > self.position.x
            && cursor_x < (self.position.x + width)
            && cursor_y as f32 > self.position.y
            && cursor_y < (self.position.y + height)
    }

    fn value_from_cursor(&self, cursor_x: f32) -> u32 {
        let value_ratio = (cursor_x as f32 - self.position.x) / (self.size.x as f32);
        let value_almost = value_ratio * self.steps as f32 - Self::DISPLAY_OFFSET;

        value_almost.round().min(self.steps as f32 - 1f32).max(0f32) as u32
    }
}

impl<'callback_ref> ElementFunctions<'callback_ref> for Slider<'callback_ref> {
    fn draw(&self, target: &mut Frame, context: &DrawContext) {
        use glium::{Blend, BlendingFunction, LinearBlendingFactor};

        let width = self.size.x;
        let height = self.size.y;

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
        // Draw all the bars (step_bg) at the background of the slider
        let bar_width = self.size.x / self.steps as f32;
        let bar_scale = Matrix4::from_nonuniform_scale(bar_width, height, 1.0);
        for (i, &has_bg) in self.step_bg.iter().enumerate() {
            if has_bg {
                let bar_pos = Vector3::new(
                    self.position.x + bar_width * i as f32,
                    self.position.y,
                    0.0
                );
                let mut transform = bar_scale;
                transform = Matrix4::from_translation(bar_pos) * transform;
                transform = context.projection_transform * transform;
                let uniforms = uniform! {
                    matrix: Into::<[[f32; 4]; 4]>::into(transform),
                    color: self.step_bg_color,
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
            }
        }

        // -----------------------
        // Draw vertical line at slider value
        // Do this before the shadow so the shadow we draw later will cover this line as well
        let value_ratio = (self.value as f32 + Self::DISPLAY_OFFSET) / (self.steps as f32);
        let slider_pos = Vector3::new(
            self.position.x + value_ratio * self.size.x,
            self.position.y,
            0.0,
        );
        let color = [0.25, 0.25, 0.25, 1.0f32];

        let mut transform = Matrix4::from_nonuniform_scale(1.0, height, 1.0);
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
            shadow_color: Into::<[f32; 3]>::into(self.shadow_color),
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

    fn handle_event(&mut self, event: &Event) -> Option<Rc<Fn() -> () + 'callback_ref>> {
        let mut result: Option<Rc<Fn() -> ()>> = None;
        match event {
            Event::MouseButton {
                button,
                state,
                position,
            } => {
                if *button == glutin::MouseButton::Left {
                    if self.cursor_above(position) {
                        if *state == glutin::ElementState::Pressed {
                            self.click = true;
                            self.value = self.value_from_cursor(position.x as f32);
                            result = Some(self.callback.clone());
                        } else {
                            self.click = false;
                        }
                    } else {
                        self.click = false;
                    }
                }
            }
            Event::MouseMove { position } => {
                self.hover = self.cursor_above(position);
                if self.click == true {
                    self.value = self.value_from_cursor(position.x as f32);
                    result = Some(self.callback.clone());
                }
            }
        }

        result
    }
}
