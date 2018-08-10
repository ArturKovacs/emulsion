use std::boxed::Box;
use std::rc::Rc;

use glium;
use glium::glutin;
use glium::texture::SrgbTexture2d;
use glium::{Frame, Surface};

use cgmath::{Matrix4, Vector2};

use ui::{DrawContext, ElementFunctions, Event};

pub struct Toggle<'a> {
    texture_on: Rc<SrgbTexture2d>,
    texture_off: Rc<SrgbTexture2d>,
    callback: Box<Fn(bool) -> () + 'a>,
    position: Vector2<f32>,
    is_on: bool,
    hover: bool,
    click: bool,
}

impl<'a> Toggle<'a> {
    pub fn new(
        texture_on: Rc<SrgbTexture2d>,
        texture_off: Rc<SrgbTexture2d>,
        callback: Box<Fn(bool) -> () + 'a>,
        position: Vector2<f32>,
        is_on: bool,
    ) -> Self {
        Toggle {
            texture_on,
            texture_off,
            callback,
            position,
            is_on,
            hover: false,
            click: false,
        }
    }

    pub fn position(&self) -> Vector2<f32> {
        self.position
    }

    pub fn set_position(&mut self, pos: Vector2<f32>) {
        self.position = pos;
    }

    pub fn set_callback(&mut self, callback: Box<Fn(bool) -> () + 'a>) {
        self.callback = callback;
    }

    fn cursor_above(&self, cursor_position: &glutin::dpi::LogicalPosition) -> bool {
        let cursor_x = cursor_position.x as f32;
        let cursor_y = cursor_position.y as f32;

        let img_w = self.texture_on.width() as f32;
        let img_h = self.texture_on.height() as f32;

        cursor_x as f32 > self.position.x
            && cursor_x < (self.position.x + img_w)
            && cursor_y as f32 > self.position.y
            && cursor_y < (self.position.y + img_h)
    }
}

impl<'a> ElementFunctions for Toggle<'a> {
    fn draw(&self, target: &mut Frame, context: &DrawContext) {
        use glium::{Blend, BlendingFunction, LinearBlendingFactor};

        let texture = if self.is_on {
            &self.texture_on
        } else {
            &self.texture_off
        };

        let img_w = texture.width() as f32;
        let img_h = texture.height() as f32;

        // Model tranform
        let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
        let transform = Matrix4::from_translation(self.position.extend(0.0)) * transform;
        // Projection
        let transform = context.projection_transform * transform;

        let sampler = texture
            .sampled()
            .wrap_function(glium::uniforms::SamplerWrapFunction::Clamp)
            .magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest);

        let texture_size = [img_w, img_h];
        // building the uniforms
        let uniforms = uniform! {
            matrix: Into::<[[f32; 4]; 4]>::into(transform),
            tex: sampler,
            texture_size: texture_size,
            brighten: if self.hover { 0.15f32 } else { 0.0f32 },
            shadow_offset: if self.click { 0.7f32 } else { 0.8f32 }
        };
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
        target
            .draw(
                context.unit_quad_vertices,
                context.unit_quad_indices,
                context.textured_program,
                &uniforms,
                &image_draw_params,
            )
            .unwrap();
    }

    fn handle_event(&mut self, event: &Event) {
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
                        } else if self.click == true {
                            self.is_on = !self.is_on;
                            (self.callback)(self.is_on);
                            self.click = false;
                        }
                    } else {
                        self.click = false;
                    }
                }
            }
            Event::MouseMove { position } => {
                self.hover = self.cursor_above(position);
            }
        }
    }
}
