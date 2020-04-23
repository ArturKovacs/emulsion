use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use crate::shaders;

use crate::playback_manager::*;

use gelatin::cgmath::{Matrix4, Vector3};
use gelatin::glium::glutin::event::{ElementState, MouseButton};
use gelatin::glium::{program, texture::SrgbTexture2d, uniform, Display, Frame, Program, Surface};

use gelatin::add_common_widget_functions;
use gelatin::line_layout_container::HorizontalLayoutContainer;
use gelatin::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use gelatin::window::Window;
use gelatin::NextUpdate;
use gelatin::{DrawContext, Event, EventKind, Widget, WidgetData, WidgetError};

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
enum HoverState {
	None,
	ItemHovered { prev_path: PathBuf },
}

struct PictureWidgetData {
	placement: WidgetPlacement,
	drawn_bounds: LogicalRect,
	prev_draw_size: LogicalVector,
	visible: bool,
	rendered_valid: bool,

	click: bool,
	hover: bool,

	playback_manager: PlaybackManager,

	program: Program,
	bright_shade: f32,
	img_texel_size: f32,
	/// Size of an image texel in physical display pixels
	image_fit: bool,
	img_pos: LogicalVector,

	last_click_time: Instant,
	last_mouse_pos: LogicalVector,
	panning: bool,
	hover_state: HoverState,

	first_draw: bool,
	next_update: NextUpdate,
	slider: Rc<gelatin::slider::Slider>,
	bottom_panel: Rc<HorizontalLayoutContainer>,
	window: Weak<Window>,
}
impl WidgetData for PictureWidgetData {
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
impl PictureWidgetData {
	fn fit_image_to_panel(&mut self, _display: &Display, dpi_scale: f32) {
		let size = self.drawn_bounds.size.vec;
		if let Some(texture) = self.get_texture() {
			let panel_aspect = size.x / size.y;
			let img_aspect = texture.width() as f32 / texture.height() as f32;

			let texel_size_to_fit_width = size.x / texture.width() as f32;
			let img_texel_size = if img_aspect > panel_aspect {
				// The image is relatively wider than the panel
				texel_size_to_fit_width
			} else {
				texel_size_to_fit_width * (img_aspect / panel_aspect)
			};
			self.img_pos = LogicalVector::new(size.x as f32 * 0.5, size.y as f32 * 0.5);
			self.img_texel_size = img_texel_size * dpi_scale;
			self.image_fit = true;
		}
	}

	fn zoom_image(&mut self, anchor: LogicalVector, image_texel_size: f32) {
		self.img_pos = (image_texel_size / self.img_texel_size) * (self.img_pos - anchor) + anchor;
		self.img_texel_size = image_texel_size;
	}

	fn update_image_transform(&mut self, display: &Display, dpi_scale: f32) {
		if self.image_fit {
			self.fit_image_to_panel(display, dpi_scale);
		} else {
			let center_offset = (self.drawn_bounds.size - self.prev_draw_size) * 0.5f32;
			self.img_pos += center_offset;
		}
		self.prev_draw_size = self.drawn_bounds.size;
	}

	fn set_window_title_filename<T: AsRef<str>>(window: &Window, name: T) {
		let title = format!("{} : E M U L S I O N", name.as_ref());
		let display = window.display_mut();
		display.gl_window().window().set_title(title.as_ref());
	}

	fn get_texture(&self) -> Option<Rc<SrgbTexture2d>> {
		self.playback_manager.image_texture().clone()
	}
}

pub struct PictureWidget {
	data: RefCell<PictureWidgetData>,
}
impl PictureWidget {
	pub fn new(
		display: &Display,
		window: &Rc<Window>,
		slider: Rc<gelatin::slider::Slider>,
		bottom_panel: Rc<HorizontalLayoutContainer>,
	) -> PictureWidget {
		let program = program!(display,
			140 => {
				vertex: shaders::VERTEX_140,
				fragment: shaders::FRAGMENT_140
			},

			110 => {
				vertex: shaders::VERTEX_110,
				fragment: shaders::FRAGMENT_110
			},
		)
		.unwrap();
		PictureWidget {
			data: RefCell::new(PictureWidgetData {
				placement: Default::default(),
				drawn_bounds: Default::default(),
				visible: true,
				prev_draw_size: Default::default(),
				click: false,
				hover: false,
				//image_texture: None,
				playback_manager: PlaybackManager::new(),
				rendered_valid: false,

				program,
				bright_shade: 0.95,
				img_texel_size: 0.0,
				image_fit: true,
				img_pos: Default::default(),
				last_click_time: Instant::now() - Duration::from_secs(10),
				last_mouse_pos: Default::default(),
				panning: false,
				hover_state: HoverState::None,
				first_draw: true,
				next_update: NextUpdate::Latest,
				slider,
				bottom_panel,
				window: Rc::downgrade(window),
			}),
		}
	}

	add_common_widget_functions!(data);

	pub fn set_bright_shade(&self, shade: f32) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.bright_shade = shade;
		borrowed.rendered_valid = false;
	}

	pub fn jump_to_index(&self, index: u32) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.playback_manager.request_load(LoadRequest::LoadAtIndex(index as usize));
		borrowed.rendered_valid = false;
	}

	pub fn jump_to_path<P: Into<PathBuf>>(&self, path: P) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.playback_manager.request_load(LoadRequest::FilePath(path.into()));
		borrowed.rendered_valid = false;
	}
}

impl Widget for PictureWidget {
	fn is_valid(&self) -> bool {
		let borrowed = self.data.borrow();
		borrowed.rendered_valid && borrowed.playback_manager.should_sleep()
	}

	fn before_draw(&self, window: &Window) {
		let mut data = self.data.borrow_mut();
		if !data.visible {
			return;
		}
		if data.first_draw {
			// Don't block on the main thread and
			// wait on the image to be loaded on the first draw,
			// instead let the ui draw itself first and then we can wait.
			data.first_draw = false;
			data.next_update = NextUpdate::Soonest;
			return;
		}
		data.next_update = data.playback_manager.update_image(window);
		let curr_file_index = data.playback_manager.current_file_index() as u32;
		let curr_dir_len = data.playback_manager.current_dir_len() as u32;
		data.slider.set_steps(curr_dir_len, curr_file_index);
		//data.slider.set_step_bg(data.playback_manager.cached_from_dir());
		match data.playback_manager.filename() {
			Some(name) => {
				PictureWidgetData::set_window_title_filename(window, name.to_str().unwrap());
			}
			None => {
				PictureWidgetData::set_window_title_filename(window, "[ none ]");
			}
		}
	}

	fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<NextUpdate, WidgetError> {
		let texture;
		{
			let mut data = self.data.borrow_mut();
			if !data.visible {
				return Ok(data.next_update);
			}
			data.update_image_transform(context.display, context.dpi_scale_factor);
			texture = data.get_texture();
		}
		{
			let data = self.data.borrow();

			let size = data.drawn_bounds.size.vec;
			let projection_transform = gelatin::cgmath::ortho(0.0, size.x, size.y, 0.0, -1.0, 1.0);

			let image_draw_params = gelatin::glium::DrawParameters {
				viewport: Some(context.logical_rect_to_viewport(&data.drawn_bounds)),
				..Default::default()
			};

			if let Some(texture) = texture {
				let img_w = texture.width() as f32;
				let img_h = texture.height() as f32;

				let img_height_over_width = img_h / img_w;
				let image_display_width = data.img_texel_size * img_w / context.dpi_scale_factor;

				// Model tranform
				let image_display_height = image_display_width * img_height_over_width;
				let img_pyhs_pos = data.img_pos.vec * context.dpi_scale_factor;
				let img_phys_siz = LogicalVector::new(image_display_width, image_display_height)
					* context.dpi_scale_factor;
				let corner_x =
					(img_pyhs_pos.x - img_phys_siz.vec.x * 0.5).floor() / context.dpi_scale_factor;
				let corner_y =
					(img_pyhs_pos.y - img_phys_siz.vec.y * 0.5).floor() / context.dpi_scale_factor;
				let transform =
					Matrix4::from_nonuniform_scale(image_display_width, image_display_height, 1.0);
				let transform =
					Matrix4::from_translation(Vector3::new(corner_x, corner_y, 0.0)) * transform;
				// Projection tranform
				let transform = projection_transform * transform;

				let sampler = texture
					.sampled()
					.wrap_function(gelatin::glium::uniforms::SamplerWrapFunction::Clamp);
				let sampler = if data.img_texel_size >= 4f32 {
					sampler.magnify_filter(gelatin::glium::uniforms::MagnifySamplerFilter::Nearest)
				} else {
					sampler.magnify_filter(gelatin::glium::uniforms::MagnifySamplerFilter::Linear)
				};
				// building the uniforms

				let uniforms = uniform! {
					matrix: Into::<[[f32; 4]; 4]>::into(transform),
					bright_shade: data.bright_shade,
					tex: sampler
				};
				target
					.draw(
						context.unit_quad_vertices,
						context.unit_quad_indices,
						&data.program,
						&uniforms,
						&image_draw_params,
					)
					.unwrap();
			}
		}
		let mut borrowed = self.data.borrow_mut();
		borrowed.rendered_valid = true;
		Ok(borrowed.next_update)
	}

	fn layout(&self, available_space: LogicalRect) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.default_layout(available_space);
	}

	fn handle_event(&self, event: &Event) {
		if !self.data.borrow().visible {
			return;
		}
		match event.kind {
			EventKind::MouseMove => {
				let mut borrowed = self.data.borrow_mut();
				borrowed.hover = borrowed.drawn_bounds.contains(event.cursor_pos);
				if borrowed.panning {
					let delta = event.cursor_pos - borrowed.last_mouse_pos;
					borrowed.image_fit = false;
					borrowed.img_pos += delta;
					borrowed.rendered_valid = false;
				}
				borrowed.last_mouse_pos = event.cursor_pos;
			}
			EventKind::MouseButton { state, button, .. } => match button {
				MouseButton::Left => {
					let mut borrowed = self.data.borrow_mut();
					if state == ElementState::Pressed {
						if borrowed.hover {
							let now = Instant::now();
							let duration_since_last_click =
								now.duration_since(borrowed.last_click_time);
							borrowed.last_click_time = now;
							if duration_since_last_click < Duration::from_millis(250) {
								// TODO
								//borrowed.toggle_fullscreen(window, bottom_panel);
								match borrowed.window.upgrade() {
									Some(window) => {
										let fullscreen = !window.fullscreen();
										window.set_fullscreen(fullscreen);
										borrowed.bottom_panel.set_visible(!fullscreen);
									}
									None => unreachable!(),
								}
							}
						}
					}
					borrowed.rendered_valid = false;
				}
				MouseButton::Right => {
					let mut borrowed = self.data.borrow_mut();
					if state == ElementState::Pressed {
						borrowed.click = borrowed.hover;
						borrowed.panning = borrowed.hover;
					} else {
						borrowed.panning = false;
						borrowed.click = false;
					}
					borrowed.rendered_valid = false;
				}
				_ => {}
			},
			EventKind::MouseScroll { delta } => {
				let mut borrowed = self.data.borrow_mut();
				let delta = delta.vec.y * 0.375;
				let delta = if delta > 0.0 { delta + 1.0 } else { 1.0 / (delta.abs() + 1.0) };

				let new_image_texel_size = (borrowed.img_texel_size * delta).max(0.0);

				borrowed.zoom_image(event.cursor_pos, new_image_texel_size);
				borrowed.image_fit = false;
			}
			EventKind::KeyInput { input } => {
				use gelatin::glium::glutin::event::VirtualKeyCode;
				if let Some(key) = input.virtual_keycode {
					let mut borrowed = self.data.borrow_mut();
					if input.state == ElementState::Pressed {
						if event.modifiers.alt() {
							match key {
								VirtualKeyCode::V | VirtualKeyCode::A => {
									match borrowed.playback_manager.playback_state() {
										PlaybackState::Forward => {
											borrowed.playback_manager.pause_playback()
										}
										_ => borrowed.playback_manager.start_playback_forward(),
									}
								}
								VirtualKeyCode::P => {
									borrowed.playback_manager.start_random_presentation();
								}
								_ => (),
							}
						} else {
							match key {
								VirtualKeyCode::Left | VirtualKeyCode::A => {
									borrowed
										.playback_manager
										.request_load(LoadRequest::LoadPrevious);
								}
								VirtualKeyCode::Right | VirtualKeyCode::D => {
									borrowed.playback_manager.request_load(LoadRequest::LoadNext);
								}
								VirtualKeyCode::F => borrowed.image_fit = true,
								VirtualKeyCode::Q => {
									borrowed.image_fit = false;
									borrowed.img_texel_size = 1.0;
								}
								VirtualKeyCode::P => {
									borrowed.playback_manager.start_presentation();
								}
								VirtualKeyCode::Space => {
									borrowed.panning = borrowed.hover;
								}
								VirtualKeyCode::Delete => {
									let path = borrowed.playback_manager.current_file_path();
									if let Err(e) = trash::remove(&path) {
										eprintln!(
											"Error while moving file '{:?}' to trash {:?}",
											path, e
										);
									}
									if let Err(e) = borrowed.playback_manager.update_directory() {
										eprintln!("Error while updating directory {:?}", e);
									}
									borrowed.rendered_valid = false;
								}
								_ => (),
							}
						}
					} else if key == VirtualKeyCode::Space {
						borrowed.panning = false;
					}
				}
			}
			EventKind::DroppedFile(ref path) => {
				let mut borrowed = self.data.borrow_mut();
				borrowed.playback_manager.request_load(LoadRequest::FilePath(path.clone()));
				borrowed.hover_state = HoverState::None;
				borrowed.rendered_valid = false;
			}
			EventKind::HoveredFile(ref path) => {
				let mut borrowed = self.data.borrow_mut();
				match borrowed.hover_state {
					HoverState::None => {
						borrowed.hover_state = HoverState::ItemHovered {
							prev_path: borrowed.playback_manager.current_file_path(),
						};
					}
					HoverState::ItemHovered { .. } => {}
				}
				borrowed.playback_manager.request_load(LoadRequest::FilePath(path.clone()));
				borrowed.rendered_valid = false;
			}
			EventKind::HoveredFileCancelled => {
				let mut borrowed = self.data.borrow_mut();
				match borrowed.hover_state.clone() {
					HoverState::None => unreachable!(),
					HoverState::ItemHovered { prev_path } => {
						borrowed.playback_manager.request_load(LoadRequest::FilePath(prev_path));
						borrowed.hover_state = HoverState::None;
					}
				}
				borrowed.rendered_valid = false;
			}
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
}
