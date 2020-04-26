//! Idk man

pub use cgmath;
pub use glium;
pub use image;

use cgmath::{Matrix4, Vector3};
use glium::glutin;
use glium::{
	implement_vertex, uniform, Blend, BlendingFunction, Display, Frame, IndexBuffer,
	LinearBlendingFactor, Program, Rect, Surface, VertexBuffer,
};
use glutin::event_loop::ControlFlow;
use std::any::Any;
use std::error::Error;
use std::fmt;
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
use std::vec::Vec;

use misc::*;

pub mod application;
pub mod button;
pub mod label;
pub mod line_layout_container;
pub mod misc;
pub mod picture;
pub mod shaders;
pub mod slider;
pub mod window;

#[derive(Debug)]
pub enum WidgetError {
	Image(image::ImageError),
	Custom(Box<dyn Error>),
}
impl fmt::Display for WidgetError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			WidgetError::Image(img_err) => write!(f, "WidgetError: Image ({})", img_err)?,
			WidgetError::Custom(err) => write!(f, "WidgetError: Custom ({})", err)?,
		}
		Ok(())
	}
}
impl Error for WidgetError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			WidgetError::Image(img_err) => Some(img_err),
			WidgetError::Custom(err) => Some(Deref::deref(err)),
		}
	}
}
impl From<image::ImageError> for WidgetError {
	fn from(img_err: image::ImageError) -> WidgetError {
		WidgetError::Image(img_err)
	}
}

pub trait WidgetData {
	fn placement(&mut self) -> &mut WidgetPlacement;

	/// The area that this widget visually occupies placed relative to the top left corner of the
	/// window in logical pixels. This area does not include the widget's margins.
	fn drawn_bounds(&mut self) -> &mut LogicalRect;

	fn visible(&mut self) -> &mut bool;

	fn apply_horizontal_alignement(&mut self, available_space: LogicalRect, width: f32) {
		self.drawn_bounds().pos.vec.x = available_space.pos.vec.x;
		match self.placement().horizontal_align {
			Alignment::Start => {
				self.drawn_bounds().pos.vec.x += self.placement().margin_left;
			}
			Alignment::Center => {
				let space_between_margins = available_space.size.vec.x
					- self.placement().margin_left
					- self.placement().margin_right;
				self.drawn_bounds().pos.vec.x +=
					self.placement().margin_left + space_between_margins * 0.5 - width * 0.5;
			}
			Alignment::End => {
				self.drawn_bounds().pos.vec.x =
					available_space.right() - (self.placement().margin_right + width);
			}
		}
	}
	fn apply_vertical_alignement(&mut self, available_space: LogicalRect, height: f32) {
		self.drawn_bounds().pos.vec.y = available_space.pos.vec.y;
		match self.placement().vertical_align {
			Alignment::Start => {
				self.drawn_bounds().pos.vec.y += self.placement().margin_top;
			}
			Alignment::Center => {
				let space_between_margins = available_space.size.vec.y
					- self.placement().margin_top
					- self.placement().margin_bottom;
				self.drawn_bounds().pos.vec.y +=
					self.placement().margin_top + space_between_margins * 0.5 - height * 0.5;
			}
			Alignment::End => {
				self.drawn_bounds().pos.vec.y =
					available_space.bottom() - (self.placement().margin_bottom + height);
			}
		}
	}
	fn default_layout(&mut self, available_space: LogicalRect) {
		if !*self.visible() {
			*self.drawn_bounds() = LogicalRect {
				pos: LogicalVector::new(0.0, 0.0),
				size: LogicalVector::new(0.0, 0.0),
			};
			return;
		}
		*self.drawn_bounds() = available_space;
		match self.placement().width {
			Length::Fixed(width) => {
				self.drawn_bounds().size.vec.x = width;
				self.apply_horizontal_alignement(available_space, width);
			}
			Length::Stretch { min, max } => {
				let mut width = available_space.size.vec.x;
				width -= self.placement().margin_left + self.placement().margin_right;
				width = width.max(min).min(max);
				self.drawn_bounds().size.vec.x = width;
				if width < max {
					self.apply_horizontal_alignement(available_space, width);
				} else {
					self.drawn_bounds().pos.vec.x += self.placement().margin_left;
				}
			}
		}
		match self.placement().height {
			Length::Fixed(height) => {
				self.drawn_bounds().size.vec.y = height;
				self.apply_vertical_alignement(available_space, height);
			}
			Length::Stretch { min, max } => {
				let mut height = available_space.size.vec.y;
				height -= self.placement().margin_top + self.placement().margin_bottom;
				height = height.max(min).min(max);
				self.drawn_bounds().size.vec.y = height;
				if height > max {
					self.apply_vertical_alignement(available_space, height);
				}
				self.drawn_bounds().pos.vec.y += self.placement().margin_top;
			}
		}
	}
}

#[derive(Copy, Clone)]
pub enum NextUpdate {
	/// Analogous to glutin::ControlFlow::Poll
	Soonest,

	/// Analogous to glutin::ControlFlow::WaitUntil
	WaitUntil(Instant),

	/// Analogous to glutin::ControlFlow::Wait
	Latest,
}

impl NextUpdate {
	pub fn aggregate(self, other: NextUpdate) -> NextUpdate {
		match other {
			NextUpdate::Soonest => other,
			NextUpdate::WaitUntil(others_time) => match self {
				NextUpdate::Soonest => self,
				NextUpdate::WaitUntil(self_time) => {
					if others_time < self_time {
						other
					} else {
						self
					}
				}
				NextUpdate::Latest => other,
			},
			NextUpdate::Latest => self,
		}
	}
}

impl Into<ControlFlow> for NextUpdate {
	fn into(self) -> ControlFlow {
		match self {
			NextUpdate::Soonest => ControlFlow::Poll,
			NextUpdate::WaitUntil(time) => ControlFlow::WaitUntil(time),
			NextUpdate::Latest => ControlFlow::Wait,
		}
	}
}

pub trait Widget: Any {
	/// When this is false, the window containing the widget
	/// will be re-rendered entirely and will run a new event loop
	/// immediately after this one, without sleeping.
	fn is_valid(&self) -> bool;

	/// This function is called before calling the draw function.
	/// Widgets may use this function to mutate the window. This is however not allowed in the
	/// `draw` method.
	///
	/// Note that the `Window` uses inner mutability so all window related functions take a
	/// reference to a seemingly immutable window.
	fn before_draw(&self, _window: &window::Window) {}

	/// This function is called when the window is being re-rendered.
	///
	/// WARNING: The window may not be modified from this function. See the `before_draw` function
	/// to do that.
	///
	/// The widget is responsible for setting the correct transformation.
	/// A widget should get information for finding a proper
	/// transformation from its own `drawn_bounds` field.
	///
	/// On success this furnction may return an instant indicating the time when it would like to
	/// be redrawn. Otherwise it can return Ok(None) to indicate that it should only be redrawn when
	/// a window event causes a change.
	fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<NextUpdate, WidgetError>;

	fn layout(&self, available_space: LogicalRect);

	fn handle_event(&self, event: &Event);

	/// The implementer is expected to `push` its children into the provided vector.
	fn children(&self, children: &mut Vec<Rc<dyn Widget>>);

	fn placement(&self) -> WidgetPlacement;

	fn visible(&self) -> bool;
}

#[macro_export]
macro_rules! add_common_widget_functions {
    ($data_field:ident) => {
        pub fn set_margin_all(&self, pixels: f32) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.margin_left = pixels;
            borrowed.placement.margin_right = pixels;
            borrowed.placement.margin_top = pixels;
            borrowed.placement.margin_bottom = pixels;
            borrowed.rendered_valid = false;
        }

        pub fn set_margin_left(&self, pixels: f32) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.margin_left = pixels;
            borrowed.rendered_valid = false;
        }
        pub fn set_margin_right(&self, pixels: f32) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.margin_right = pixels;
            borrowed.rendered_valid = false;
        }
        pub fn set_margin_top(&self, pixels: f32) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.margin_top = pixels;
            borrowed.rendered_valid = false;
        }
        pub fn set_margin_bottom(&self, pixels: f32) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.margin_bottom = pixels;
            borrowed.rendered_valid = false;
        }
        pub fn set_horizontal_align(&self, align: Alignment) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.horizontal_align = align;
            borrowed.rendered_valid = false;
        }
        pub fn set_vertical_align(&self, align: Alignment) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.vertical_align = align;
            borrowed.rendered_valid = false;
        }
        pub fn set_fixed_size(&self, size: LogicalVector) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.width = Length::Fixed(size.vec.x);
            borrowed.placement.height = Length::Fixed(size.vec.y);
            borrowed.rendered_valid = false;
        }
        pub fn set_width(&self, width: Length) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.width = width;
            borrowed.rendered_valid = false;
        }
        pub fn set_height(&self, height: Length) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.height = height;
            borrowed.rendered_valid = false;
        }
        pub fn set_ignore_layout(&self, ignore: bool) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.placement.ignore_layout = ignore;
            borrowed.rendered_valid = false;
        }
        pub fn set_visible(&self, visible: bool) {
            let mut borrowed = self.$data_field.borrow_mut();
            borrowed.visible = visible;
            borrowed.rendered_valid = false;
        }
    };
}

pub struct Event {
	/// The position of the cursor in virtual pixels
	/// relative to the top left corner of the window.
	pub cursor_pos: LogicalVector,
	pub modifiers: glutin::event::ModifiersState,
	pub kind: EventKind,
}
pub enum EventKind {
	MouseMove,
	MouseButton { state: glutin::event::ElementState, button: glutin::event::MouseButton },
	MouseScroll { delta: LogicalVector },
	KeyInput { input: glutin::event::KeyboardInput },
	ReceivedCharacter(char),
	DroppedFile(PathBuf),
	HoveredFile(PathBuf),
	HoveredFileCancelled,
}

#[derive(Copy, Clone)]
pub struct Vertex {
	pub position: [f32; 2],
	pub tex_coords: [f32; 2],
}

#[allow(clippy::unneeded_field_pattern)]
implement_vertex!(Vertex, position, tex_coords);

pub struct DrawContext<'a> {
	pub display: &'a Display,
	pub dpi_scale_factor: f32,
	pub unit_quad_vertices: &'a VertexBuffer<Vertex>,
	pub unit_quad_indices: &'a IndexBuffer<u16>,
	pub textured_program: &'a Program,
	pub colored_shadowed_program: &'a Program,
	pub colored_program: &'a Program,
	pub viewport: &'a Rect,
	pub projection_transform: &'a Matrix4<f32>,
}
impl<'a> DrawContext<'a> {
	pub fn logical_rect_to_viewport(&self, rect: &LogicalRect) -> Rect {
		let dpi_scale = self.dpi_scale_factor;
		let window_phys_height = self.viewport.height;
		Rect {
			left: (rect.pos.vec.x * dpi_scale) as u32,
			width: (rect.size.vec.x * dpi_scale) as u32,
			bottom: window_phys_height - (rect.bottom() * dpi_scale) as u32,
			height: (rect.size.vec.y * dpi_scale) as u32,
		}
	}
	pub fn clear_color(&self, target: &mut Frame, color: [f32; 4], rect: Option<LogicalRect>) {
		// Rendering a quad to emulate clear.
		// This is a workaround for https://github.com/glium/glium/issues/1842

		let transform;
		if let Some(rect) = rect {
			let width = rect.size.vec.x;
			let height = rect.size.vec.y;
			// Model tranform
			let scale = Matrix4::from_nonuniform_scale(width, height, 1.0);
			let translate = Matrix4::from_translation(rect.pos.vec.extend(0.0)) * scale;
			transform = self.projection_transform * translate;
		} else {
			let scale = Matrix4::from_scale(2.0);
			transform = Matrix4::from_translation(Vector3::new(-1.0, -1.0, 0.0)) * scale;
		}
		let image_draw_params = glium::DrawParameters {
			blend: Blend {
				color: BlendingFunction::Addition {
					source: LinearBlendingFactor::SourceAlpha,
					destination: LinearBlendingFactor::OneMinusSourceAlpha,
				},
				..Default::default()
			},
			..Default::default()
		};
		let uniforms = uniform! {
			matrix: Into::<[[f32; 4]; 4]>::into(transform),
			color: color,
		};
		target
			.draw(
				self.unit_quad_vertices,
				self.unit_quad_indices,
				self.colored_program,
				&uniforms,
				&image_draw_params,
			)
			.unwrap();
	}
}
