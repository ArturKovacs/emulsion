
use std::rc::Rc;
use std::boxed::Box;

use glium;
use glium::{Surface, Frame};
use glium::texture::SrgbTexture2d;
use glium::glutin;

use cgmath::{Matrix4, Vector2};

use ui::{Element, DrawContext, Event};


pub struct Button {
    texture: Rc<SrgbTexture2d>,
    callback: Box<Fn() -> ()>,
    position: Vector2<f32>,
    hover: bool
}

impl Button {
    pub fn new(
        texture: Rc<SrgbTexture2d>,
        callback: Box<Fn() -> ()>,
        position: Vector2<f32>,
    ) -> Button {
        Button {
            texture,
            callback,
            position,
            hover: false
        }
    }

    fn cursor_above(&self, cursor_position: &glutin::dpi::LogicalPosition) -> bool {
        let cursor_x = cursor_position.x as f32;
        let cursor_y = cursor_position.y as f32;

        let img_w = self.texture.width() as f32;
        let img_h = self.texture.height() as f32;

        cursor_x as f32 > self.position.x && cursor_x < (self.position.x + img_w)
            && cursor_y as f32 > self.position.y && cursor_y < (self.position.y + img_h)
    }
}

impl Element for Button {

    fn draw(&self, target: &mut Frame, context: &DrawContext) {
        use glium::{Blend, BlendingFunction, LinearBlendingFactor};

        let img_w = self.texture.width() as f32;
        let img_h = self.texture.height() as f32;

        // Model tranform
        let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
        let transform = Matrix4::from_translation(-1.0 * self.position.extend(0.0)) * transform;
        // Projection
        let transform = context.projection_transform * transform;

        let sampler = self.texture
            .sampled()
            .wrap_function(glium::uniforms::SamplerWrapFunction::Clamp)
            .magnify_filter(glium::uniforms::MagnifySamplerFilter::Nearest);
      
        // building the uniforms
        let uniforms = uniform! {
            matrix: Into::<[[f32; 4]; 4]>::into(transform),
            tex: sampler,
            brighten: if self.hover { 0.25f32 } else { 0.0f32 }
        };
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
        target
            .draw(
                context.unit_quad_vertices,
                context.unit_quad_indices,
                context.program,
                &uniforms,
                &image_draw_params,
            )
            .unwrap();
    }


    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseButton {button, state, position } => {
                if *button == glutin::MouseButton::Left && *state == glutin::ElementState::Pressed {
                    if self.cursor_above(position) {
                        (self.callback)();
                    }
                }
            }
            Event::MouseMove {position} => {
                self.hover = self.cursor_above(position);
            }
        }
    }
}
