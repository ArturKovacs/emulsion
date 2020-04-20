use std::cell::RefCell;
use std::rc::Rc;

use cgmath::{Matrix4, Vector3};
use glium::{uniform, Frame, Surface};

use crate::add_common_widget_functions;
use crate::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use crate::picture::Picture;
use crate::NextUpdate;
use crate::{DrawContext, Event, Widget, WidgetData, WidgetError};

struct LabelData {
	placement: WidgetPlacement,
	drawn_bounds: LogicalRect,
	visible: bool,

	icon: Option<Rc<Picture>>,

	rendered_valid: bool,
}
impl WidgetData for LabelData {
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

pub struct Label {
	data: RefCell<LabelData>,
}

impl Label {
	pub fn new() -> Label {
		Label {
			data: RefCell::new(LabelData {
				placement: Default::default(),
				drawn_bounds: Default::default(),
				visible: true,
				icon: None,
				rendered_valid: false,
			}),
		}
	}

	add_common_widget_functions!(data);

	pub fn set_icon(&self, img: Option<Rc<Picture>>) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.icon = img;
		borrowed.rendered_valid = false;
	}
}

impl Widget for Label {
	fn is_valid(&self) -> bool {
		self.data.borrow().rendered_valid
	}

	fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<NextUpdate, WidgetError> {
		use glium::{Blend, BlendingFunction, LinearBlendingFactor};
		{
			let borrowed = self.data.borrow();

			let aligned_bounds = borrowed.drawn_bounds.align_to_pixels(context.dpi_scale_factor);

			let img_w = aligned_bounds.size.vec.x;
			let img_h = aligned_bounds.size.vec.y;

			// Model tranform
			let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
			let transform =
				Matrix4::from_translation(aligned_bounds.pos.vec.extend(0.0)) * transform;
			// Projection
			let transform = context.projection_transform * transform;

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
			let texture_size = [img_w, img_h];
			if let Some(ref icon) = borrowed.icon {
				let texture = icon.texture(context.display)?;
				let sampler = texture
					.sampled()
					.wrap_function(glium::uniforms::SamplerWrapFunction::Clamp)
					.minify_filter(glium::uniforms::MinifySamplerFilter::Linear)
					.magnify_filter(glium::uniforms::MagnifySamplerFilter::Linear);
				let uniforms = uniform! {
					matrix: Into::<[[f32; 4]; 4]>::into(transform),
					tex: sampler,
					color: [1.0f32, 0.1, 0.5, 0.5],
					texture_size: texture_size,
					//brighten: if self.hover { 0.15f32 } else { 0.0f32 },
					brighten: 0.0f32,
					shadow_color: Into::<[f32; 3]>::into(Vector3::<f32>::new(0.0, 0.0, 0.0)),
					shadow_offset: 1.0f32,
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
			} else {
				// building the uniforms
				let uniforms = uniform! {
					matrix: Into::<[[f32; 4]; 4]>::into(transform),
					color: [1.0f32, 0.1, 0.5, 0.5],
					size: texture_size,
					//brighten: if self.hover { 0.15f32 } else { 0.0f32 },
					brighten: 0.0f32,
					shadow_color: Into::<[f32; 3]>::into(Vector3::<f32>::new(0.0, 0.0, 0.0)),
					shadow_offset: 1.0f32,
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
		}
		self.data.borrow_mut().rendered_valid = true;
		Ok(NextUpdate::Latest)
	}

	fn layout(&self, available_space: LogicalRect) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.default_layout(available_space);
	}

	fn handle_event(&self, _event: &Event) {}

	// No children for a button
	fn children(&self, _children: &mut Vec<Rc<dyn Widget>>) {}

	fn placement(&self) -> WidgetPlacement {
		self.data.borrow().placement
	}

	fn visible(&self) -> bool {
		self.data.borrow().visible
	}
}
