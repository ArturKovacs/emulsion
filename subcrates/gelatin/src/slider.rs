use std::{cell::RefCell, rc::Rc};

use cgmath::{Matrix4, Vector3};
use glium::{uniform, Frame, Surface};
use winit::event::{ElementState, MouseButton};

use crate::add_common_widget_functions;
use crate::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use crate::window::RenderValidity;
use crate::NextUpdate;
use crate::{DrawContext, Event, EventKind, Widget, WidgetData, WidgetError};

struct SliderData {
	placement: WidgetPlacement,
	drawn_bounds: LogicalRect,
	visible: bool,

	steps: u32,
	value: u32,
	click: bool,
	hover: bool,
	on_value_change: Option<Rc<dyn Fn()>>,
	shadow_color: [f32; 3],

	render_validity: RenderValidity,
	//rendered_valid: bool,
}
impl WidgetData for SliderData {
	fn placement(&mut self) -> &mut WidgetPlacement {
		&mut self.placement
	}
	fn drawn_bounds(&mut self) -> &mut LogicalRect {
		&mut self.drawn_bounds
	}
	fn visible(&mut self) -> &mut bool {
		&mut self.visible
	}
}

pub struct Slider {
	data: RefCell<SliderData>,
}

impl Slider {
	pub fn new() -> Slider {
		Slider {
			data: RefCell::new(SliderData {
				placement: Default::default(),
				drawn_bounds: Default::default(),
				visible: true,
				steps: 1,
				value: 0,
				click: false,
				hover: false,
				on_value_change: None,
				shadow_color: [0.0, 0.0, 0.0],
				render_validity: Default::default(),
				//rendered_valid: false,
			}),
		}
	}

	add_common_widget_functions!(data);

	pub fn steps(&self) -> u32 {
		self.data.borrow().steps
	}

	pub fn value(&self) -> u32 {
		self.data.borrow().value
	}

	pub fn set_steps(&self, steps: u32, value: u32) {
		let mut borrowed = self.data.borrow_mut();
		let prev_steps = borrowed.steps;
		let prev_value = borrowed.value;
		borrowed.steps = steps;
		borrowed.value = value;
		if prev_steps != steps || prev_value != value {
			borrowed.render_validity.invalidate();
		}
	}

	pub fn set_value(&self, value: u32) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.value = value;
		borrowed.render_validity.invalidate();
	}

	/// Feel free to use `RefCell`s within the callback to satisfy the apparent constnes
	/// of the callback.
	pub fn set_on_value_change<T: Fn() + 'static>(&self, callback: T) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.on_value_change = Some(Rc::new(callback));
	}

	pub fn set_shadow_color(&self, color: [f32; 3]) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.shadow_color = color;
		borrowed.render_validity.invalidate();
	}
}

impl Widget for Slider {
	fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<NextUpdate, WidgetError> {
		use glium::{Blend, BlendingFunction, LinearBlendingFactor};
		{
			let borrowed = self.data.borrow();
			if !borrowed.visible {
				return Ok(NextUpdate::Latest);
			}
			let aligned_bounds = borrowed.drawn_bounds.align_to_pixels(context.dpi_scale_factor);
			let position = aligned_bounds.pos.vec;
			let size = aligned_bounds.size.vec;
			//let width = borrowed.drawn_bounds.size.vec.x;
			//let height = borrowed.drawn_bounds.size.vec.y;

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
			// Draw vertical line at slider value
			// Do this before the shadow so the shadow we draw later will cover this line as well
			let value_ratio = (borrowed.value as f32 + 0.5) / (borrowed.steps as f32);
			let slider_pos = Vector3::new(position.x + value_ratio * size.x, position.y, 0.0);
			let color = [0.4, 0.4, 0.4, 1.0f32];

			let mut transform = Matrix4::from_nonuniform_scale(1.0, size.y, 1.0);
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
			// Model tranform
			let transform = Matrix4::from_nonuniform_scale(size.x, size.y, 1.0);
			let transform = Matrix4::from_translation(position.extend(0.0)) * transform;
			// Projection
			let transform = context.projection_transform * transform;

			// building the uniforms
			let uniforms = uniform! {
				matrix: Into::<[[f32; 4]; 4]>::into(transform),
				color: [0.0f32, 0.0, 0.0, 0.0],
				size: [size.x, size.y],
				brighten: 0.0f32,
				shadow_color: borrowed.shadow_color,
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
		Ok(NextUpdate::Latest)
	}

	fn layout(&self, available_space: LogicalRect) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.default_layout(available_space);
	}

	fn handle_event(&self, event: &Event) {
		if !self.data.borrow().visible {
			return;
		}
		let check_value_change = || {
			// We jugle around the `on_value_change` callback so that when it gets called,
			// `self.data` is not borrowed.
			let on_value_change;
			{
				let mut borrowed = self.data.borrow_mut();
				borrowed.hover = borrowed.drawn_bounds.contains(event.cursor_pos);
				if borrowed.click {
					let prev_value = borrowed.value;
					let relative_cursor_x =
						event.cursor_pos.vec.x - borrowed.drawn_bounds.pos.vec.x;
					let proportion =
						(relative_cursor_x / borrowed.drawn_bounds.size.vec.x).clamp(0.0, 1.0);
					let stepsf = borrowed.steps as f32;
					borrowed.value =
						(proportion * (1.0 + 1.0 / stepsf) * (stepsf - 1.0)).floor() as u32;
					if borrowed.value != prev_value {
						borrowed.render_validity.invalidate();
						on_value_change = borrowed.on_value_change.clone();
					} else {
						on_value_change = None;
					}
				} else {
					on_value_change = None;
				}
			}
			if let Some(callback) = on_value_change {
				callback();
			}
		};
		match event.kind {
			EventKind::MouseMove => {
				check_value_change();
			}
			EventKind::MouseButton { state, button: MouseButton::Left, .. } => match state {
				ElementState::Pressed => {
					{
						let mut borrowed = self.data.borrow_mut();
						borrowed.click = borrowed.hover;
					}
					check_value_change();
				}
				ElementState::Released => {
					let mut borrowed = self.data.borrow_mut();
					borrowed.click = false;
				}
			},
			_ => (),
		}
	}

	// No children for a button
	fn children(&self, _children: &mut Vec<Rc<dyn Widget>>) {}

	fn placement(&self) -> WidgetPlacement {
		self.data.borrow().placement
	}

	fn visible(&self) -> bool {
		self.data.borrow().visible
	}

	fn set_valid_ref(&self, render_validity: RenderValidity) {
		self.data.borrow_mut().render_validity = render_validity;
	}
}

impl Default for Slider {
	fn default() -> Self {
		Self::new()
	}
}
