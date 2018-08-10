use std::boxed::Box;
use std::rc::Rc;

use glium::glutin;
use glium::texture::SrgbTexture2d;
use glium::{Display, DrawParameters, Frame, IndexBuffer, Program, Rect, Surface, VertexBuffer};

use cgmath::{Matrix4, Vector2};

mod button;
use ui::button::Button;

mod toggle;
use ui::toggle::Toggle;

mod slider;
use ui::slider::Slider;

use shaders;

#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

implement_vertex!(Vertex, position, tex_coords);

pub struct DrawContext<'a> {
    unit_quad_vertices: &'a VertexBuffer<Vertex>,
    unit_quad_indices: &'a IndexBuffer<u16>,
    textured_program: &'a Program,
    colored_shadowed_program: &'a Program,
    colored_program: &'a Program,
    viewport: &'a Rect,
    projection_transform: &'a Matrix4<f32>,
}

pub enum Event {
    MouseButton {
        button: glutin::MouseButton,
        state: glutin::ElementState,
        position: glutin::dpi::LogicalPosition,
    },
    MouseMove {
        position: glutin::dpi::LogicalPosition,
    },
}

pub trait ElementFunctions {
    fn draw(&self, target: &mut Frame, context: &DrawContext);
    fn handle_event(&mut self, event: &Event);
}

#[derive(Copy, Clone)]
pub struct ButtonId<'a> {
    ptr: *mut Button<'a>,
}

#[derive(Copy, Clone)]
pub struct ToggleId<'a> {
    ptr: *mut Toggle<'a>,
}

#[derive(Copy, Clone)]
pub struct SliderId<'a> {
    ptr: *mut Slider<'a>,
}

pub struct Ui<'a> {
    buttons: Vec<Box<Button<'a>>>,
    toggles: Vec<Box<Toggle<'a>>>,
    sliders: Vec<Box<Slider<'a>>>,
    unit_quad_vertices: VertexBuffer<Vertex>,
    unit_quad_indices: IndexBuffer<u16>,
    textured_program: Program,
    colored_shadowed_program: Program,
    colored_program: Program,
    cursor_pos: glutin::dpi::LogicalPosition,
    height: f32,
}

impl<'reference, 'element: 'reference> Ui<'element> {
    pub fn new(display: &Display, height: f32) -> Self {
        use glium::index::PrimitiveType;

        let vertex_buffer = {
            VertexBuffer::new(
                display,
                &[
                    Vertex {
                        position: [0.0, 0.0],
                        tex_coords: [0.0, 1.0],
                    },
                    Vertex {
                        position: [0.0, 1.0],
                        tex_coords: [0.0, 0.0],
                    },
                    Vertex {
                        position: [1.0, 1.0],
                        tex_coords: [1.0, 0.0],
                    },
                    Vertex {
                        position: [1.0, 0.0],
                        tex_coords: [1.0, 1.0],
                    },
                ],
            ).unwrap()
        };

        // building the index buffer
        let index_buffer =
            IndexBuffer::new(display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3]).unwrap();

        // compiling shaders and linking them together
        let textured_program = program!(display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::UI_FRAGMENT_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::UI_FRAGMENT_110
            },
        ).unwrap();

        let colored_shadowed_program = program!(display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::COLOR_SHADOW_F_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::COLOR_SHADOW_F_110
            },
        ).unwrap();

        let colored_program = program!(display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::COLOR_F_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::COLOR_F_110
            },
        ).unwrap();

        Ui {
            buttons: Vec::new(),
            toggles: Vec::new(),
            sliders: Vec::new(),
            unit_quad_vertices: vertex_buffer,
            unit_quad_indices: index_buffer,
            textured_program,
            colored_shadowed_program,
            colored_program,
            cursor_pos: glutin::dpi::LogicalPosition::new(0.0, 0.0),
            height,
        }
    }

    pub fn window_event(
        &mut self,
        event: &glutin::WindowEvent,
        window_size: glutin::dpi::LogicalSize,
    ) {
        let event = match event {
            glutin::WindowEvent::CursorMoved { position, .. } => {
                self.cursor_pos.x = position.x;
                self.cursor_pos.y = window_size.height as f64 - position.y;

                Event::MouseMove {
                    position: self.cursor_pos,
                }
            }
            glutin::WindowEvent::MouseInput { state, button, .. } => Event::MouseButton {
                button: *button,
                state: *state,
                position: self.cursor_pos,
            },
            _ => return,
        };

        for button in self.buttons.iter_mut() {
            button.handle_event(&event);
        }
        for toggle in self.toggles.iter_mut() {
            toggle.handle_event(&event);
        }
        for slider in self.sliders.iter_mut() {
            slider.handle_event(&event);
        }
    }

    pub fn draw(&self, target: &mut Frame, bg_color: &[f32; 4]) {
        use cgmath::ortho;

        let width = target.get_dimensions().0 as f32;

        let left = 0f32;
        let right = width + left;
        let bottom = 0f32;
        let top = self.height + bottom;
        let projection_transform = ortho(left, right, bottom, top, -1f32, 1f32);

        let viewport = Rect {
            left: left as u32,
            width: width as u32,
            bottom: bottom as u32,
            height: self.height as u32,
        };

        let context = DrawContext {
            unit_quad_vertices: &self.unit_quad_vertices,
            unit_quad_indices: &self.unit_quad_indices,
            textured_program: &self.textured_program,
            colored_shadowed_program: &self.colored_shadowed_program,
            colored_program: &self.colored_program,
            viewport: &viewport,
            projection_transform: &projection_transform,
        };

        Self::draw_background(target, &context, bg_color);

        for button in self.buttons.iter() {
            button.draw(target, &context);
        }
        for toggle in self.toggles.iter() {
            toggle.draw(target, &context);
        }
        for slider in self.sliders.iter() {
            slider.draw(target, &context);
        }
    }

    pub fn get_button_mut(
        &'reference mut self,
        id: ButtonId<'element>,
    ) -> Option<&'reference mut Button<'element>> {
        for button in self.buttons.iter_mut() {
            let button = &mut (**button);
            let ptr = button as *mut Button;
            if ptr == id.ptr {
                return Some(button);
            }
        }
        None
    }

    pub fn get_toggle_mut(
        &'reference mut self,
        id: ToggleId<'element>,
    ) -> Option<&'reference mut Toggle<'element>> {
        for toggle in self.toggles.iter_mut() {
            let mut toggle = &mut (**toggle);
            let ptr = toggle as *mut Toggle;
            if ptr == id.ptr {
                return Some(toggle);
            }
        }
        None
    }

    pub fn get_slider_mut(
        &'reference mut self,
        id: SliderId<'element>,
    ) -> Option<&'reference mut Slider<'element>> {
        for slider in self.sliders.iter_mut() {
            let mut slider = &mut (**slider);
            let ptr = slider as *mut Slider;
            if ptr == id.ptr {
                return Some(slider);
            }
        }
        None
    }

    pub fn create_button(
        &mut self,
        texture: Rc<SrgbTexture2d>,
        position: Vector2<f32>,
        callback: Box<Fn() -> () + 'element>,
    ) -> ButtonId<'element> {
        let mut result = Box::new(Button::new(texture, callback, position));

        let ptr = &mut (*result) as *mut Button;

        self.buttons.push(result);

        ButtonId { ptr }
    }

    pub fn create_toggle(
        &mut self,
        texture_on: Rc<SrgbTexture2d>,
        texture_off: Rc<SrgbTexture2d>,
        position: Vector2<f32>,
        is_on: bool,
        callback: Box<Fn(bool) -> () + 'element>,
    ) -> ToggleId<'element> {
        let mut result = Box::new(Toggle::new(
            texture_on,
            texture_off,
            callback,
            position,
            is_on,
        ));

        let ptr = &mut (*result) as *mut Toggle;

        self.toggles.push(result);

        ToggleId { ptr }
    }

    pub fn create_slider(
        &mut self,
        position: Vector2<f32>,
        size: Vector2<f32>,
        steps: u32,
        value: u32,
        callback: Box<Fn(u32, u32) -> () + 'element>,
    ) -> SliderId<'element> {
        let mut result = Box::new(Slider::new(position, size, steps, value, callback));

        let ptr = &mut (*result) as *mut Slider;

        self.sliders.push(result);

        SliderId { ptr }
    }

    fn draw_background(target: &mut Frame, context: &DrawContext, color: &[f32; 4]) {
        let image_draw_params = DrawParameters {
            viewport: Some(*context.viewport),
            ..Default::default()
        };

        let mut transform = Matrix4::from_nonuniform_scale(
            context.viewport.width as f32,
            context.viewport.height as f32,
            1.0,
        );
        transform = context.projection_transform * transform;
        let uniforms = uniform! {
            matrix: Into::<[[f32; 4]; 4]>::into(transform),
            color: *color,
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
