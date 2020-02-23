use glium::glutin::{self, dpi::PhysicalSize, event::WindowEvent, window::WindowId};
use glium::{program, IndexBuffer, Program, Rect, Surface, VertexBuffer};

use std::cell::RefCell;
use std::cmp::Eq;
use std::convert::AsRef;
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

#[derive(Clone)]
struct WidgetReference {
    pub widget: Rc<dyn Widget>,
}
impl Deref for WidgetReference {
    type Target = Rc<dyn Widget>;
    fn deref(&self) -> &Self::Target {
        &self.widget
    }
}
impl DerefMut for WidgetReference {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.widget
    }
}
impl Hash for WidgetReference {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.widget.as_ref() as *const dyn Widget).hash(state);
    }
}
impl PartialEq for WidgetReference {
    fn eq(&self, other: &WidgetReference) -> bool {
        Rc::ptr_eq(&self.widget, &other.widget)
    }
}
impl Eq for WidgetReference {}

struct WindowData {
    display: glium::Display,
    //size_before_fullscreen: PhysicalSize<i32>,
    //fullscreen: bool,
    redraw_needed: bool,
    cursor_pos: LogicalVector,
    //widgets: HashSet<WidgetReference>,
    root_widget: Option<Rc<dyn Widget>>,

    // Draw data
    unit_quad_vertices: VertexBuffer<Vertex>,
    unit_quad_indices: IndexBuffer<u16>,
    textured_program: Program,
    colored_shadowed_program: Program,
    colored_program: Program,
}
#[derive(Clone)]
pub struct Window {
    data: Rc<RefCell<WindowData>>,
    //widgets_clone: RefCell<Vec<WidgetReference>>,
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
    pub fn new(application: &mut Application) -> Self {
        //use glium::glutin::window::Icon;
        //let exe_parent = std::env::current_exe().unwrap().parent().unwrap().to_owned();

        let window_size = PhysicalSize::<i32>::new(200, 600);

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
                    Vertex { position: [0.0, 0.0], tex_coords: [0.0, 1.0] },
                    Vertex { position: [0.0, 1.0], tex_coords: [0.0, 0.0] },
                    Vertex { position: [1.0, 1.0], tex_coords: [1.0, 0.0] },
                    Vertex { position: [1.0, 0.0], tex_coords: [1.0, 1.0] },
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

        let resulting_window = Window {
            data: Rc::new(RefCell::new(WindowData {
                display,
                // size_before_fullscreen: window_size,
                // fullscreen: false,
                cursor_pos: Default::default(),
                redraw_needed: false,
                //widgets: HashSet::new(),
                root_widget: None,

                unit_quad_vertices: vertex_buffer,
                unit_quad_indices: index_buffer,
                textured_program,
                colored_shadowed_program,
                colored_program,
            })),
            //widgets_clone: RefCell::new(Vec::new()),
        };

        application.register_window(&resulting_window);

        resulting_window
    }

    pub fn set_root<T: Widget>(&self, widget: Option<Rc<T>>) {
        let mut borrowed = self.data.borrow_mut();
        match widget {
            Some(w) => borrowed.root_widget = Some(w),
            None => borrowed.root_widget = None,
        }
        //borrowed.root_widget = Some(widget.take());
    }

    //pub fn add_widget<T: Widget>(&self, widget: Rc<T>) {
    //    let mut borrowed = self.data.borrow_mut();
    //    borrowed.widgets.insert(WidgetReference { widget });
    //}

    //pub fn remove_widget<T: Widget>(&self, widget: Rc<T>) {
    //    let mut borrowed = self.data.borrow_mut();
    //    borrowed.widgets.remove(&WidgetReference { widget });
    //}

    pub fn process_event(&self, native_event: WindowEvent) {
        use glutin::event::MouseScrollDelta;
        // let mut widgets_clone = self.widgets_clone.borrow_mut();
        // widgets_clone.clear();
        // for widget in self.data.borrow().widgets.iter() {
        //     widgets_clone.push(widget.clone());
        // }
        // // Doing this widget clone jugling just to
        // // free the window from being borrowed while the events are being handled
        // let mut redraw_needed = false;
        // for widget in widgets_clone.iter() {
        //     widget.handle_event(&event);
        //     if !widget.is_valid() {
        //         redraw_needed = true;
        //     }
        // }
        // self.data.borrow_mut().redraw_needed = redraw_needed;

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
                            delta = LogicalVector::new(x * 12.0, y * 8.0);
                        }
                        MouseScrollDelta::PixelDelta(native_pos) => {
                            delta = LogicalVector::new(native_pos.x as f32, native_pos.y as f32);
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
                _ => event = None,
            }
        }

        if let Some(event) = event {
            let cloned;
            if let Some(ref widget) = self.data.borrow().root_widget {
                cloned = Some(widget.clone());
            } else {
                cloned = None;
            }
            if let Some(widget) = cloned {
                widget.handle_event(&event);
                self.data.borrow_mut().redraw_needed = !widget.is_valid();
            }
        }
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
        let mut target = self.data.borrow_mut().display.draw();
        let dpi_scaling = self.data.borrow_mut().display.gl_window().window().scale_factor();
        target.clear_color(0.0, 0.1, 0.1, 1.0);

        // let mut widgets_clone = self.widgets_clone.borrow_mut();
        // widgets_clone.clear();
        // for widget in self.data.borrow().widgets.iter() {
        //     widgets_clone.push(widget.clone());
        // }

        let cloned;
        if let Some(ref widget) = self.data.borrow().root_widget {
            cloned = Some(widget.clone());
        } else {
            cloned = None;
        }

        let dimensions = target.get_dimensions();
        let phys_dimensions =
            glutin::dpi::PhysicalSize::new(dimensions.0 as f32, dimensions.1 as f32);
        let phys_width = phys_dimensions.width;
        let phys_height = phys_dimensions.height;
        let logical_dimensions = LogicalVector::from_physical(phys_dimensions, dpi_scaling as f32);

        // Invoke the layout functions
        let available_widget_space =
            LogicalRect { pos: LogicalVector::new(0.0, 0.0), size: logical_dimensions };
        if let Some(ref widget) = cloned {
            widget.layout(available_widget_space);
        }

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
        let draw_context = DrawContext {
            unit_quad_vertices: &borrowed.unit_quad_vertices,
            unit_quad_indices: &borrowed.unit_quad_indices,
            textured_program: &borrowed.textured_program,
            colored_shadowed_program: &borrowed.colored_shadowed_program,
            colored_program: &borrowed.colored_program,
            viewport: &viewport,
            projection_transform: &projection_transform,
        };

        if let Some(ref widget) = cloned {
            widget.draw(&mut target, &draw_context);
        }

        //target.clear();
        target.finish().unwrap();
    }
}
