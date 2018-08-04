
use std::mem;
use std::rc::Rc;
use std::cell::RefCell;
use std::boxed::Box;

use glium::{Display, Rect, Frame, Surface, VertexBuffer, IndexBuffer, Program};
use glium::texture::SrgbTexture2d;
use glium::glutin;

use cgmath::{Matrix4, Vector2};

mod button;
use ui::button::Button;

mod toggle;
use ui::toggle::Toggle;

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

#[derive(Copy, Clone)]
pub struct ToggleId<'a> {
    ptr: *mut Toggle<'a>
}

pub struct Ui<'a> {
    buttons: Vec<Box<Button<'a>>>,
    toggles: Vec<Box<Toggle<'a>>>,
    unit_quad_vertices: VertexBuffer<Vertex>,
    unit_quad_indices: IndexBuffer<u16>,
    program: Program,
    cursor_pos: glutin::dpi::LogicalPosition
}

impl<'reference, 'element: 'reference> Ui<'element> {
    pub fn new(display: &Display) -> Self {
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
            buttons: Vec::new(),
            toggles: Vec::new(),
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

        for button in self.buttons.iter_mut() {
            button.handle_event(&event);
        }
        for toggle in self.toggles.iter_mut() {
            toggle.handle_event(&event);
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

        for button in self.buttons.iter() {
            button.draw(target, &context);
        }
        for toggle in self.toggles.iter() {
            toggle.draw(target, &context);
        }
    }


    pub fn get_button_mut(&'reference mut self, id: ButtonId<'element>)
    -> Option<&'reference mut Button<'element>> {
        for button in self.buttons.iter_mut() {
            let button = &mut (**button);
            let ptr = button as *mut Button;
            if ptr == id.ptr {
                return Some(button);
            }
        }
        None
    }


    pub fn get_toggle_mut(&'reference mut self, id: ToggleId<'element>)
    -> Option<&'reference mut Toggle<'element>> {
        for toggle in self.toggles.iter_mut() {
            let mut toggle = &mut (**toggle);
            let ptr = toggle as *mut Toggle;
            if ptr == id.ptr {
                return Some(toggle);
            }
        }
        None
    }


    pub fn create_button(
        &mut self,
        texture: Rc<SrgbTexture2d>,
        position: Vector2<f32>,
        callback: Box<Fn() -> () + 'element>
    ) -> ButtonId<'element> {
        let mut result = Box::new(Button::new(
            texture, callback, position,
        ));

        let ptr = &mut (*result) as *mut Button;

        self.buttons.push(result);

        ButtonId {
            ptr
        }
    }


    pub fn create_toggle(
        &mut self,
        texture_on: Rc<SrgbTexture2d>,
        texture_off: Rc<SrgbTexture2d>,
        position: Vector2<f32>,
        is_on: bool,
        callback: Box<Fn(bool) -> () + 'element>
    ) -> ToggleId<'element> {
        let mut result = Box::new(Toggle::new(
            texture_on, texture_off, callback, position, is_on
        ));

        let ptr = &mut (*result) as *mut Toggle;

        self.toggles.push(result);

        ToggleId {
            ptr
        }
    }
}
