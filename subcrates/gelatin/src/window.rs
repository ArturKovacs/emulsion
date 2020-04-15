use glium::glutin::{self, dpi::PhysicalSize, event::WindowEvent, window::WindowId};
use glium::{program, IndexBuffer, Program, Rect, Surface, VertexBuffer, Display};

use std::cell::{RefCell, RefMut};
use std::cmp::Eq;
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use cgmath::ortho;

use crate::application::Application;
use crate::shaders;
use crate::{
    misc::{FromPhysical, LogicalRect, LogicalVector},
    DrawContext, Event, EventKind, Vertex, Widget,
};

pub struct WindowDisplayRefMut<'a> {
    window_ref: RefMut<'a, WindowData>,
}
impl<'a> Deref for WindowDisplayRefMut<'a> {
    type Target = Display;
    fn deref(&self) -> &Self::Target {
        &self.window_ref.display
    }
}
impl<'a> DerefMut for WindowDisplayRefMut<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.window_ref.display
    }
}

struct WindowData {
    display: glium::Display,
    size_before_fullscreen: PhysicalSize<u32>,
    fullscreen: bool,
    redraw_needed: bool,
    cursor_pos: LogicalVector,
    root_widget: Rc<dyn Widget>,
    bg_color: [f32; 4],

    // Draw data
    unit_quad_vertices: VertexBuffer<Vertex>,
    unit_quad_indices: IndexBuffer<u16>,
    textured_program: Program,
    colored_shadowed_program: Program,
    colored_program: Program,
}

pub struct Window {
    data: RefCell<WindowData>,
}
impl Hash for Window {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.data.as_ptr().hash(state);
    }
}
impl PartialEq for Window {
    fn eq(&self, other: &Window) -> bool {
        self.data.as_ptr() == other.data.as_ptr()
    }
}
impl Eq for Window {}

impl Window {
    pub fn new(application: &mut Application) -> Rc<Self> {
        //use glium::glutin::window::Icon;
        //let exe_parent = std::env::current_exe().unwrap().parent().unwrap().to_owned();

        let window_size = PhysicalSize::<u32>::new(800, 600);

        let window = glutin::window::WindowBuilder::new()
            .with_title("Loading")
            .with_fullscreen(None)
            .with_inner_size(window_size)
            //.with_window_icon(Some(icon))
            .with_visible(true);

        let context = glutin::ContextBuilder::new().with_gl_profile(glutin::GlProfile::Core);
        let display = glium::Display::new(window, context, &application.event_loop).unwrap();

        // display.gl_window().window().set_ime_position(LogicalPosition::new(
        //     config.window_x as f64,
        //     config.window_y as f64,
        // ));
        //display.gl_window().window().set_visible(true);

        // All the draw stuff
        use glium::index::PrimitiveType;
        let vertex_buffer = {
            VertexBuffer::new(
                &display,
                &[
                    Vertex { position: [0.0, 0.0], tex_coords: [0.0, 0.0] },
                    Vertex { position: [0.0, 1.0], tex_coords: [0.0, 1.0] },
                    Vertex { position: [1.0, 1.0], tex_coords: [1.0, 1.0] },
                    Vertex { position: [1.0, 0.0], tex_coords: [1.0, 0.0] },
                ],
            )
            .unwrap()
        };

        // building the index buffer
        let index_buffer =
            IndexBuffer::new(&display, PrimitiveType::TriangleStrip, &[1 as u16, 2, 0, 3]).unwrap();

        // compiling shaders and linking them together
        let textured_program = program!(&display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::TEXTURE_SHADOW_F_140
            },
            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::TEXTURE_SHADOW_F_110
            },
        )
        .unwrap();
        let colored_shadowed_program = program!(&display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::COLOR_SHADOW_F_140
            },
            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::COLOR_SHADOW_F_110
            },
        )
        .unwrap();
        let colored_program = program!(&display,
            140 => {
                vertex: shaders::VERTEX_140,
                fragment: shaders::COLOR_F_140
            },
            110 => {
                vertex: shaders::VERTEX_110,
                fragment: shaders::COLOR_F_110
            },
        )
        .unwrap();

        let resulting_window = Rc::new(Window {
            data: RefCell::new(WindowData {
                display,
                size_before_fullscreen: window_size,
                fullscreen: false,
                cursor_pos: Default::default(),
                redraw_needed: false,
                //widgets: HashSet::new(),
                root_widget: Rc::new(crate::line_layout_container::VerticalLayoutContainer::new()),
                bg_color: [0.85, 0.85, 0.85, 1.0],

                unit_quad_vertices: vertex_buffer,
                unit_quad_indices: index_buffer,
                textured_program,
                colored_shadowed_program,
                colored_program,
            }),
        });

        application.register_window(resulting_window.clone());
        resulting_window
    }

    pub fn set_root<T: Widget>(&self, widget: Rc<T>) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.root_widget = widget;
        borrowed.redraw_needed = true;
    }

    pub fn set_bg_color(&self, color: [f32; 4]) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.bg_color = color;
    }

    pub fn process_event(&self, native_event: WindowEvent) {
        use glutin::event::MouseScrollDelta;

        let event;
        {
            let mut borrowed = self.data.borrow_mut();
            match native_event {
                WindowEvent::KeyboardInput { input, .. } => {
                    event = Some(Event {
                        cursor_pos: borrowed.cursor_pos,
                        kind: EventKind::KeyInput { input },
                    });
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let logical_pos;
                    {
                        let gl_window = borrowed.display.gl_window();
                        let window = gl_window.window();

                        let scaling = window.scale_factor() as f32;

                        logical_pos = LogicalVector::from_physical(position, scaling);
                        //logical_pos.vec.y = logical_dimensions.vec.y - logical_pos.vec.y;
                    }
                    borrowed.cursor_pos = logical_pos;
                    event =
                        Some(Event { cursor_pos: borrowed.cursor_pos, kind: EventKind::MouseMove });
                }
                WindowEvent::MouseWheel { delta: native_delta, .. } => {
                    let delta;
                    match native_delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            delta = LogicalVector::new(x, y);
                        }
                        MouseScrollDelta::PixelDelta(native_pos) => {
                            delta = LogicalVector::new(
                                native_pos.x as f32 / 13.0, native_pos.y as f32 / 8.0
                            );
                        }
                    }
                    event = Some(Event {
                        cursor_pos: borrowed.cursor_pos,
                        kind: EventKind::MouseScroll { delta },
                    });
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    event = Some(Event {
                        cursor_pos: borrowed.cursor_pos,
                        kind: EventKind::MouseButton { state, button },
                    });
                }
                WindowEvent::DroppedFile(path) => {
                    event = Some(Event {
                        cursor_pos: borrowed.cursor_pos,
                        kind: EventKind::DroppedFile(path),
                    });
                }
                WindowEvent::HoveredFile(path) => {
                    event = Some(Event {
                        cursor_pos: borrowed.cursor_pos,
                        kind: EventKind::HoveredFile(path),
                    });
                },
                WindowEvent::HoveredFileCancelled => {
                    event = Some(Event {
                        cursor_pos: borrowed.cursor_pos,
                        kind: EventKind::HoveredFileCancelled,
                    });
                },
                _ => event = None,
            }
        }

        if let Some(event) = event {
            let cloned = self.data.borrow().root_widget.clone();
            cloned.handle_event(&event);
            self.data.borrow_mut().redraw_needed = !cloned.is_valid();
        }
    }

    pub fn display_mut<'a>(&'a self) -> WindowDisplayRefMut<'a> {
        WindowDisplayRefMut { window_ref: self.data.borrow_mut() }
    }

    pub fn get_id(&self) -> WindowId {
        self.data.borrow().display.gl_window().window().id()
    }

    pub fn request_redraw(&self) {
        self.data.borrow_mut().display.gl_window().window().request_redraw();
    }

    pub fn redraw_needed(&self) -> bool {
        // TODO return true if any of the components
        // `is_valid` returns false
        true
    }

    /// WARNING The window may not be changed during the drawing phase.
    /// This means that trying to borrow the window *mutably* in a widget's
    /// draw function will fail.
    pub fn redraw(&self) {
        let root_widget = self.data.borrow().root_widget.clone();
        // this way self.data is not borrowed while before draw is running.
        root_widget.before_draw(self);
        let mut target = self.data.borrow_mut().display.draw();
        let dpi_scaling = self.data.borrow_mut().display.gl_window().window().scale_factor();

        let dimensions = target.get_dimensions();
        let phys_dimensions =
            glutin::dpi::PhysicalSize::new(dimensions.0 as f32, dimensions.1 as f32);
        let phys_width = phys_dimensions.width;
        let phys_height = phys_dimensions.height;
        let logical_dimensions = LogicalVector::from_physical(phys_dimensions, dpi_scaling as f32);

        // Invoke the layout functions
        let available_widget_space =
            LogicalRect { pos: LogicalVector::new(0.0, 0.0), size: logical_dimensions };
        root_widget.layout(available_widget_space);

        let left = 0f32;
        let right = logical_dimensions.vec.x;
        let bottom = logical_dimensions.vec.y;
        let top = 0f32;
        let projection_transform = ortho(left, right, bottom, top, -1f32, 1f32);

        let viewport = Rect {
            left: 0 as u32,
            width: phys_width as u32,
            bottom: 0 as u32,
            height: phys_height as u32,
        };

        // Can't change the window during drawing phase. Deal with it.
        let borrowed = self.data.borrow();
        target.clear_color(
            borrowed.bg_color[0],
            borrowed.bg_color[1],
            borrowed.bg_color[2],
            borrowed.bg_color[3]
        );
        let draw_context = DrawContext {
            display: &borrowed.display,
            dpi_scale_factor: dpi_scaling as f32,
            unit_quad_vertices: &borrowed.unit_quad_vertices,
            unit_quad_indices: &borrowed.unit_quad_indices,
            textured_program: &borrowed.textured_program,
            colored_shadowed_program: &borrowed.colored_shadowed_program,
            colored_program: &borrowed.colored_program,
            viewport: &viewport,
            projection_transform: &projection_transform,
        };

        // Using the cloned root instead of self.root_widget doesn't make much difference
        // because self is being borrowed by through the draw_context anyways but it's fine.
        root_widget.draw(&mut target, &draw_context).unwrap();

        //target.clear();
        target.finish().unwrap();
    }

    pub fn fullscreen(&self) -> bool {
        self.data.borrow().fullscreen
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        let mut borrowed = self.data.borrow_mut();
        borrowed.fullscreen = fullscreen;
        let monitor = if fullscreen {
            let curr_mon;
            borrowed.size_before_fullscreen = {
                let gl_win = borrowed.display.gl_window();
                curr_mon = gl_win.window().current_monitor();
                gl_win.window().inner_size()
            };
            Some(glutin::window::Fullscreen::Borderless(curr_mon))
        } else {
            None
        };
        let gl_win = borrowed.display.gl_window();
        gl_win.window().set_fullscreen(monitor);
    }
}
