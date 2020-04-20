use std::cell::RefCell;
use std::rc::Rc;

use gelatin::cgmath::{Matrix4, Vector3};

use gelatin::glium::{uniform, Frame, Surface};

use gelatin::add_common_widget_functions;
use gelatin::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use gelatin::picture::Picture;
use gelatin::window::Window;
use gelatin::NextUpdate;
use gelatin::{DrawContext, Event, Widget, WidgetData, WidgetError};

struct HelpScreenData {
	placement: WidgetPlacement,
	drawn_bounds: LogicalRect,
	visible: bool,
	rendered_valid: bool,
	initiaizlied: bool,
	parent_space: LogicalRect,
	usage_image: Picture,
}

impl WidgetData for HelpScreenData {
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

pub struct HelpScreen {
	data: RefCell<HelpScreenData>,
}

impl HelpScreen {
	pub fn new(usage_img: Picture) -> HelpScreen {
		let placement = WidgetPlacement {
			width: Length::Fixed(0.0 as f32),
			height: Length::Fixed(0.0 as f32),
			horizontal_align: Alignment::Center,
			vertical_align: Alignment::Center,
			ignore_layout: true,
			..Default::default()
		};
		HelpScreen {
			data: RefCell::new(HelpScreenData {
				placement,
				drawn_bounds: Default::default(),
				visible: false,
				rendered_valid: false,
				initiaizlied: false,
				parent_space: LogicalRect::default(),
				usage_image: usage_img,
			}),
		}
	}

	add_common_widget_functions!(data);
}

impl Widget for HelpScreen {
	fn is_valid(&self) -> bool {
		self.data.borrow().rendered_valid
	}

	fn before_draw(&self, _window: &Window) {}

	fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<NextUpdate, WidgetError> {
		use gelatin::glium::{Blend, BlendingFunction, LinearBlendingFactor};
		self.data.borrow_mut().rendered_valid = true;
		{
			let borrowed = self.data.borrow();
			if !borrowed.visible {
				return Ok(NextUpdate::Latest);
			}

			let w = borrowed.parent_space.size.vec.x;
			let h = borrowed.parent_space.size.vec.y;
			let pos = borrowed.parent_space.pos.vec;
			// Model tranform
			let transform = Matrix4::from_nonuniform_scale(w, h, 1.0);
			let transform = Matrix4::from_translation(pos.extend(0.0)) * transform;
			// Projection
			let transform = context.projection_transform * transform;
			let image_draw_params = gelatin::glium::DrawParameters {
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
			let uniforms = uniform! {
				matrix: Into::<[[f32; 4]; 4]>::into(transform),
				color: [0.0f32, 0.0, 0.0, 0.5],
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

			///////////////////////////////////////////////////////////////////////////
			// Draw Help Image
			//////////////////////////////////////////////////////////////////////////
			let aligned_bounds = borrowed.drawn_bounds.align_to_pixels(context.dpi_scale_factor);
			let img_w = aligned_bounds.size.vec.x;
			let img_h = aligned_bounds.size.vec.y;
			let pos = aligned_bounds.pos.vec;
			// Model tranform
			let transform = Matrix4::from_nonuniform_scale(img_w, img_h, 1.0);
			let transform = Matrix4::from_translation(pos.extend(0.0)) * transform;
			// Projection
			let transform = context.projection_transform * transform;

			let texture_size = [img_w, img_h];
			let texture = borrowed.usage_image.texture(context.display)?;
			let sampler = texture
				.sampled()
				.wrap_function(gelatin::glium::uniforms::SamplerWrapFunction::Clamp)
				.minify_filter(gelatin::glium::uniforms::MinifySamplerFilter::Linear)
				.magnify_filter(gelatin::glium::uniforms::MagnifySamplerFilter::Linear);
			let uniforms = uniform! {
				matrix: Into::<[[f32; 4]; 4]>::into(transform),
				tex: sampler,
				color: [1.0f32, 0.1, 0.5, 0.5],
				texture_size: texture_size,
				//brighten: if self.hover { 0.15f32 } else { 0.0f32 },
				brighten: 0.0f32,
				shadow_color: Into::<[f32; 3]>::into(Vector3::<f32>::new(0.0, 0.0, 0.0)),
				shadow_offset: 1.0f32
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
			//////////////////////////////////////////////////////////////////////////

			//let uniforms
		}
		Ok(NextUpdate::Latest)
	}

	fn layout(&self, available_space: LogicalRect) {
		let mut borrowed = self.data.borrow_mut();
		if !borrowed.visible {
			return;
		}
		if !borrowed.initiaizlied {
			borrowed.initiaizlied = true;
			let img_data = borrowed.usage_image.get_metadata().unwrap();
			borrowed.placement = WidgetPlacement {
				width: Length::Fixed(img_data.width as f32 * 0.5),
				height: Length::Fixed(img_data.height as f32 * 0.5),
				horizontal_align: Alignment::Center,
				vertical_align: Alignment::Center,
				ignore_layout: true,
				..Default::default()
			};
		}
		borrowed.default_layout(available_space);
		borrowed.parent_space = available_space;
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
