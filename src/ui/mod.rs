
use std::rc::Rc;
use std::cell::RefCell;
use std::boxed::Box;

use glium::{Display, Rect, Frame, Surface, VertexBuffer, IndexBuffer, Program};
use glium::texture::SrgbTexture2d;
use glium::glutin;

use cgmath::{Matrix4, Vector2};

mod button;
use ui::button::Button;

mod label;
use ui::label::Label;

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
    program: &'a Program,
    viewport: &'a Rect,
    projection_transform: &'a Matrix4<f32>,
}

pub enum Event {
    MouseButton {
        button: glutin::MouseButton,
        state: glutin::ElementState,
        position: glutin::dpi::LogicalPosition
    },
    MouseMove {
        position: glutin::dpi::LogicalPosition
    }
}

pub trait ElementFunctions {
    fn draw(&self, target: &mut Frame, context: &DrawContext);
    fn handle_event(&mut self, event: &Event);
}


#[derive(Copy, Clone)]
pub struct ButtonId<'a> {
    ptr: *mut Button<'a>
}

pub struct Ui<'a> {
    height: u32,
    elements: Vec<Box<Button<'a>>>,
    unit_quad_vertices: VertexBuffer<Vertex>,
    unit_quad_indices: IndexBuffer<u16>,
    program: Program,
    cursor_pos: glutin::dpi::LogicalPosition
}

impl<'button, 'a: 'button> Ui<'a> {
    pub fn new(display: &Display, height: u32) -> Self {
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
            IndexBuffer::new(display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3])
                .unwrap();

        // compiling shaders and linking them together
        let program = program!(display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::UI_FRAGMENT_140
            },

            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::UI_FRAGMENT_110
            },
        ).unwrap();

        Ui {
            height,
            elements: Vec::new(),
            unit_quad_vertices: vertex_buffer,
            unit_quad_indices: index_buffer,
            program,
            cursor_pos: glutin::dpi::LogicalPosition::new(0.0, 0.0)
        }
    }

    pub fn window_event(&mut self, event: &glutin::WindowEvent, window_size: glutin::dpi::LogicalSize) {
        let event = match event {
            glutin::WindowEvent::CursorMoved {position, ..} => {
                self.cursor_pos.x = position.x;
                self.cursor_pos.y = window_size.height as f64 - position.y;

                Event::MouseMove {
                    position: self.cursor_pos
                }
            },
            glutin::WindowEvent::MouseInput {state, button, ..} => {
                Event::MouseButton {
                    button: *button,
                    state: *state,
                    position: self.cursor_pos
                }
            },
            _ => return,
        };

        for element in self.elements.iter_mut() {
            element.handle_event(&event);
        }
    }

    pub fn draw(&self, target: &mut Frame) {
        use cgmath::ortho;

        let (width, height) = target.get_dimensions();

        let left = 0f32;
        let right = width as f32 + left;
        let bottom = 0f32;
        let top = height as f32 + bottom;
        let projection_transform = ortho(left, right, bottom, top, -1f32, 1f32);

        let viewport = Rect {
            left: left as u32,
            width,
            bottom: bottom as u32,
            height
        };

        let context = DrawContext {
            unit_quad_vertices: &self.unit_quad_vertices,
            unit_quad_indices: &self.unit_quad_indices,
            program: &self.program,
            viewport: &viewport,
            projection_transform: &projection_transform,
        };

        for element in self.elements.iter() {
            element.draw(target, &context);
        }
    }

    pub fn get_button_mut(&'button mut self, id: ButtonId<'a>) -> Option<&'button mut Button<'a>> {
        for element in self.elements.iter_mut() {
            let element = &mut (**element);
            let ptr = element as *mut Button;
            if ptr == id.ptr {
                return Some(element);
            }
        }

        None
    }

    pub fn create_button(
        &mut self,
        texture: Rc<SrgbTexture2d>,
        callback: fn() -> ()
    ) -> ButtonId<'a> {
        let mut result = Box::new(Button::new(
            texture, Box::new(callback), Vector2::new(0.0, 0.0),
        ));

        let ptr = &mut (*result) as *mut Button;

        self.elements.push(result);

        ButtonId {
            ptr
        }
    }
}
