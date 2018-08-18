use std::rc::Rc;

use glium;
use glium::glutin;
use glium::texture::SrgbTexture2d;
use glium::{Frame, Surface};

use cgmath::{Matrix4, Vector2, Vector3};

use ui::{DrawContext, ElementFunctions, Event};

pub struct Toggle<'callback_ref> {
    texture: Rc<SrgbTexture2d>,
    callback: Rc<Fn() + 'callback_ref>,
    position: Vector2<f32>,
    shadow_color: Vector3<f32>,
    is_on: bool,
    hover: bool,
    click: bool,
}

impl<'callback_ref> Toggle<'callback_ref> {
    pub fn new<F>(
        texture: Rc<SrgbTexture2d>,
        callback: F,
        position: Vector2<f32>,
        is_on: bool,
    ) -> Self
    where
        F: Fn() + 'callback_ref,
    {
        Toggle {
            texture,
            callback: Rc::new(callback),
            position,
            shadow_color: Vector3::new(0.0, 0.0, 0.0f32),
            is_on,
            hover: false,
            click: false,
        }
    }

    pub fn set_texture(&mut self, texture: Rc<SrgbTexture2d>) {
        self.texture = texture;
    }

    pub fn position(&self) -> Vector2<f32> {
        self.position
    }

    pub fn is_on(&self) -> bool {
        self.is_on
    }

    pub fn set_position(&mut self, pos: Vector2<f32>) {
        self.position = pos;
    }

    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn() + 'callback_ref,
    {
        self.callback = Rc::new(callback);
    }

    pub fn set_shadow_color(&mut self, color: Vector3<f32>) {
        self.shadow_color = color;
    }

    fn cursor_above(&self, cursor_position: &glutin::dpi::LogicalPosition) -> bool {
        let cursor_x = cursor_position.x as f32;
        let cursor_y = cursor_position.y as f32;

        let img_w = self.texture.width() as f32;
        let img_h = self.texture.height() as f32;

        cursor_x as f32 > self.position.x
            && cursor_x < (self.position.x + img_w)
            && cursor_y as f32 > self.position.y
            && cursor_y < (self.position.y + img_h)
    }
}

impl<'callback_ref> ElementFunctions<'callback_ref> for Toggle<'callback_ref> {
    fn draw(&self, target: &mut Frame, context: &DrawContext) {
        use glium::{Blend, BlendingFunction, LinearBlendingFactor};

        let img_w = self.texture.width() as f32;
        let img_h = self.texture.height() as f32;

        // Model tranform
        let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
        let transform = Matrix4::from_translation(self.position.extend(0.0)) * transform;
        // Projection
        let transform = context.projection_transform * transform;

        let sampler = self
            .texture
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
            shadow_color: Into::<[f32; 3]>::into(self.shadow_color),
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

    fn handle_event(&mut self, event: &Event) -> Option<Rc<Fn() + 'callback_ref>> {
        let mut result: Option<Rc<Fn()>> = None;
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
                            self.click = false;

                            result = Some(self.callback.clone());
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
        result
    }
}
