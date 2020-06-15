use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gelatin::cgmath::{Matrix4, Vector3};
use gelatin::glium::glutin::event::{ElementState, ModifiersState, MouseButton};
use gelatin::glium::{program, texture::SrgbTexture2d, uniform, Display, Frame, Program, Surface};

use gelatin::add_common_widget_functions;
use gelatin::button::Button;
use gelatin::line_layout_container::HorizontalLayoutContainer;
use gelatin::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use gelatin::slider::Slider;
use gelatin::window::{RenderValidity, Window};
use gelatin::NextUpdate;
use gelatin::{
	application::request_exit, DrawContext, Event, EventKind, Widget, WidgetData, WidgetError,
};

use crate::input_handling::*;
use crate::shaders;
use crate::utils::{virtual_keycode_is_char, virtual_keycode_to_string};
use crate::{
	configuration::{Cache, Configuration},
	playback_manager::*,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ScalingMode {
	Fixed,
	FitStretch,
	FitMin,
}

#[derive(Debug, Clone)]
enum HoverState {
	None,
	ItemHovered { prev_path: PathBuf },
}

static NO_BG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
static ACTIVE_BG_COLOR: [f32; 4] = [0.3, 0.3, 0.3, 0.5];

struct PictureWidgetData {
	placement: WidgetPlacement,
	drawn_bounds: LogicalRect,
	prev_draw_size: LogicalVector,
	visible: bool,
	render_validity: RenderValidity,

	click: bool,
	hover: bool,

	configuration: Rc<RefCell<Configuration>>,
	cache: Arc<Mutex<Cache>>,
	playback_manager: PlaybackManager,

	program: Program,
	bright_shade: f32,
	/// Size of an image texel in physical display pixels
	img_texel_size: f32,
	scaling: ScalingMode,
	img_pos: LogicalVector,

	last_click_time: Instant,
	last_mouse_pos: LogicalVector,
	panning: bool,
	hover_state: HoverState,

	first_draw: bool,
	next_update: NextUpdate,
	slider: Rc<Slider>,
	orig_scale_button: Rc<Button>,
	fit_best_button: Rc<Button>,
	fit_stretch_button: Rc<Button>,
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
	fn fit_image_to_panel(&mut self, _display: &Display, dpi_scale: f32, stretch: bool) {
		let size = self.drawn_bounds.size.vec;
		if let Some(texture) = self.get_texture() {
			let panel_aspect = size.x / size.y;
			let img_phys_w = texture.width() as f32;
			let img_pyhs_h = texture.height() as f32;
			let img_aspect = img_phys_w / img_pyhs_h;

			let texel_size_to_fit_width = size.x / texture.width() as f32;
			let img_texel_size = if img_aspect > panel_aspect {
				// The image is relatively wider than the panel
				texel_size_to_fit_width
			} else {
				texel_size_to_fit_width * (img_aspect / panel_aspect)
			};
			let widget_phys_size = size * dpi_scale;
			let fits_in_widget =
				widget_phys_size.x >= img_phys_w && widget_phys_size.y >= img_pyhs_h;
			self.img_pos = LogicalVector::new(size.x as f32 * 0.5, size.y as f32 * 0.5);
			if fits_in_widget && !stretch {
				self.img_texel_size = 1.0;
			} else {
				self.img_texel_size = img_texel_size * dpi_scale;
			}
			if stretch {
				self.scaling = ScalingMode::FitStretch;
			} else {
				self.scaling = ScalingMode::FitMin;
			}
		}
	}

	fn zoom_image(&mut self, anchor: LogicalVector, mut image_texel_size: f32) {
		if (image_texel_size - 1.0).abs() < 0.01 {
			image_texel_size = 1.0;
		}
		self.img_pos = (image_texel_size / self.img_texel_size) * (self.img_pos - anchor) + anchor;
		self.img_texel_size = image_texel_size;
		self.scaling = ScalingMode::Fixed;
		self.update_scaling_buttons();
		self.render_validity.invalidate();
	}

	fn update_image_transform(&mut self, display: &Display, dpi_scale: f32) {
		match self.scaling {
			ScalingMode::Fixed => {
				let center_offset = (self.drawn_bounds.size - self.prev_draw_size) * 0.5f32;
				self.img_pos += center_offset;
			}
			ScalingMode::FitStretch => {
				self.fit_image_to_panel(display, dpi_scale, true);
			}
			ScalingMode::FitMin => {
				self.fit_image_to_panel(display, dpi_scale, false);
			}
		}
		self.prev_draw_size = self.drawn_bounds.size;
	}

	fn set_window_title_filename(
		&self,
		window: &Window,
		playback_state: PlaybackState,
		file_path: &Option<PathBuf>,
	) {
		let playback = match playback_state {
			PlaybackState::Forward => " : Playing",
			PlaybackState::Present => " : Presenting",
			PlaybackState::RandomPresent => " : Presenting Shuffled",
			PlaybackState::Paused => "",
		};

		let config = self.configuration.borrow();
		let title_config = config.title.clone().unwrap_or_default();

		let name = match file_path {
			Some(file_path) => title_config.format_file_path(file_path),
			None => "[ none ]".into(),
		};
		let title = format!("{}{}{}", name, playback, title_config.format_program_name());
		let display = window.display_mut();
		display.gl_window().window().set_title(title.as_str());
	}

	fn get_texture(&self) -> Option<Rc<SrgbTexture2d>> {
		self.playback_manager.image_texture()
	}

	pub fn set_img_size_to_orig(&mut self) {
		self.img_texel_size = 1.0;
		self.scaling = ScalingMode::Fixed;
		self.update_scaling_buttons();
		self.render_validity.invalidate();
	}

	pub fn set_img_size_to_fit(&mut self, stretch: bool) {
		{
			let mut cache = self.cache.lock().unwrap();
			cache.image.fit_stretches = stretch;
		}
		self.scaling = if stretch { ScalingMode::FitStretch } else { ScalingMode::FitMin };
		self.update_scaling_buttons();
		self.render_validity.invalidate();
	}

	#[allow(clippy::float_cmp)]
	fn update_scaling_buttons(&mut self) {
		match self.scaling {
			ScalingMode::Fixed => {
				if self.img_texel_size == 1.0 {
					self.orig_scale_button.set_bg_color(ACTIVE_BG_COLOR);
				} else {
					self.orig_scale_button.set_bg_color(NO_BG_COLOR);
				}
				self.fit_best_button.set_bg_color(NO_BG_COLOR);
				self.fit_stretch_button.set_bg_color(NO_BG_COLOR);
			}
			ScalingMode::FitMin => {
				self.orig_scale_button.set_bg_color(NO_BG_COLOR);
				self.fit_best_button.set_bg_color(ACTIVE_BG_COLOR);
				self.fit_stretch_button.set_bg_color(NO_BG_COLOR);
			}
			ScalingMode::FitStretch => {
				self.orig_scale_button.set_bg_color(NO_BG_COLOR);
				self.fit_best_button.set_bg_color(NO_BG_COLOR);
				self.fit_stretch_button.set_bg_color(ACTIVE_BG_COLOR);
			}
		}
	}
}

pub struct PictureWidget {
	data: RefCell<PictureWidgetData>,
}
impl PictureWidget {
	pub fn new(
		display: &Display,
		window: &Rc<Window>,
		slider: Rc<Slider>,
		orig_scale_button: Rc<Button>,
		fit_best_button: Rc<Button>,
		fit_stretch_button: Rc<Button>,
		bottom_panel: Rc<HorizontalLayoutContainer>,
		configuration: Rc<RefCell<Configuration>>,
		cache: Arc<Mutex<Cache>>,
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

		let scaling;
		{
			let cache = cache.lock().unwrap();
			if cache.image.fit_stretches {
				scaling = ScalingMode::FitStretch;
			} else {
				scaling = ScalingMode::FitMin;
			}
		}

		let mut data = PictureWidgetData {
			placement: Default::default(),
			drawn_bounds: Default::default(),
			visible: true,
			prev_draw_size: Default::default(),
			click: false,
			hover: false,
			configuration,
			cache,
			playback_manager: PlaybackManager::new(),
			render_validity: Default::default(),

			program,
			bright_shade: 0.95,
			img_texel_size: 0.0,
			scaling,
			img_pos: Default::default(),
			last_click_time: Instant::now() - Duration::from_secs(10),
			last_mouse_pos: Default::default(),
			panning: false,
			hover_state: HoverState::None,
			first_draw: true,
			next_update: NextUpdate::Latest,
			slider,
			orig_scale_button,
			fit_best_button,
			fit_stretch_button,
			bottom_panel,
			window: Rc::downgrade(window),
		};
		data.update_scaling_buttons();
		PictureWidget { data: RefCell::new(data) }
	}

	add_common_widget_functions!(data);

	pub fn set_bright_shade(&self, shade: f32) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.bright_shade = shade;
		borrowed.render_validity.invalidate();
	}

	pub fn set_img_size_to_orig(&self) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.set_img_size_to_orig();
	}

	pub fn set_img_size_to_fit(&self, stretch: bool) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.set_img_size_to_fit(stretch);
	}

	pub fn jump_to_index(&self, index: u32) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.playback_manager.request_load(LoadRequest::LoadAtIndex(index as usize));
		borrowed.render_validity.invalidate();
	}

	pub fn jump_to_path<P: Into<PathBuf>>(&self, path: P) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.playback_manager.request_load(LoadRequest::FilePath(path.into()));
		borrowed.render_validity.invalidate();
	}

	fn handle_key_input(&self, input_key: &str, modifiers: ModifiersState) {
		let mut borrowed = self.data.borrow_mut();
		macro_rules! triggered {
			($action_name:ident) => {
				action_triggered(&borrowed.configuration, $action_name, input_key, modifiers)
			};
		}
		if triggered!(TOGGLE_FULLSCREEN_NAME) {
			if let Some(window) = borrowed.window.upgrade() {
				let fullscreen = !window.fullscreen();
				window.set_fullscreen(fullscreen);
				borrowed.bottom_panel.set_visible(!fullscreen);
			}
		}
		if triggered!(ESCAPE_NAME) {
			if let Some(window) = borrowed.window.upgrade() {
				if window.fullscreen() {
					window.set_fullscreen(false);
					borrowed.bottom_panel.set_visible(true);
				} else {
					request_exit();
				}
			}
		}
		if triggered!(PLAY_ANIM_NAME) {
			match borrowed.playback_manager.playback_state() {
				PlaybackState::Forward => borrowed.playback_manager.pause_playback(),
				_ => borrowed.playback_manager.start_playback_forward(),
			}
		}
		if triggered!(IMG_PREV_NAME) {
			borrowed.playback_manager.request_load(LoadRequest::LoadPrevious);
			borrowed.render_validity.invalidate();
		}
		if triggered!(IMG_NEXT_NAME) {
			borrowed.playback_manager.request_load(LoadRequest::LoadNext);
			borrowed.render_validity.invalidate();
		}
		if triggered!(IMG_FIT_NAME) {
			borrowed.set_img_size_to_fit(true);
		}
		if triggered!(IMG_FIT_BEST_NAME) {
			borrowed.set_img_size_to_fit(false);
		}
		if triggered!(IMG_ORIG_NAME) {
			borrowed.set_img_size_to_orig();
		}
		if triggered!(PLAY_PRESENT_NAME) {
			match borrowed.playback_manager.playback_state() {
				PlaybackState::Present => borrowed.playback_manager.pause_playback(),
				_ => borrowed.playback_manager.start_presentation(),
			}
			borrowed.render_validity.invalidate();
		}
		if triggered!(PLAY_PRESENT_RND_NAME) {
			match borrowed.playback_manager.playback_state() {
				PlaybackState::RandomPresent => borrowed.playback_manager.pause_playback(),
				_ => borrowed.playback_manager.start_random_presentation(),
			}
			borrowed.render_validity.invalidate();
		}
		if triggered!(IMG_DEL_NAME) {
			let path = borrowed.playback_manager.current_file_path();
			if let Err(e) = trash::remove(&path) {
				eprintln!("Error while moving file '{:?}' to trash: {:?}", path, e);
			}
			if let Err(e) = borrowed.playback_manager.update_directory() {
				eprintln!("Error while updating directory {:?}", e);
			}
			borrowed.render_validity.invalidate();
		}
		let img_path = borrowed.playback_manager.current_file_path();
		if let Some(folder_path) = img_path.parent() {
			let img_and_folder = (img_path.to_str(), folder_path.to_str());
			if let (Some(img_path), Some(folder_path)) = img_and_folder {
				execute_triggered_commands(
					borrowed.configuration.clone(),
					input_key,
					modifiers,
					img_path,
					folder_path,
				);
			} else {
				eprintln!("Could not convert the image path to utf8. Path: '{:?}'", img_path);
			}
		} else {
			eprintln!("Could not get parent folder for the image path {:?}", img_path);
		}
	}
}

impl Widget for PictureWidget {
	fn before_draw(&self, window: &Window) -> NextUpdate {
		let mut data = self.data.borrow_mut();
		if !data.visible {
			return NextUpdate::Latest;
		}
		if data.first_draw {
			// Don't block on the main thread and
			// wait on the image to be loaded on the first draw,
			// instead let the ui draw itself first and then we can wait.
			data.first_draw = false;
			data.next_update = NextUpdate::Soonest;
			return data.next_update;
		}
		let prev_texture = data.playback_manager.image_texture();
		data.next_update = data.playback_manager.update_image(window);
		let new_texture = data.playback_manager.image_texture();
		let curr_file_index = data.playback_manager.current_file_index() as u32;
		let curr_dir_len = data.playback_manager.current_dir_len() as u32;
		data.slider.set_steps(curr_dir_len, curr_file_index);
		//data.slider.set_step_bg(data.playback_manager.cached_from_dir());
		let playback_state = data.playback_manager.playback_state();
		data.set_window_title_filename(window, playback_state, data.playback_manager.file_path());
		if prev_texture.is_none() != new_texture.is_none() {
			data.render_validity.invalidate();
		} else {
			if let (Some(prev_tex), Some(new_tex)) = (prev_texture, new_texture) {
				if !Rc::ptr_eq(&prev_tex, &new_tex) {
					data.render_validity.invalidate();
				}
			}
		}
		data.next_update
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

			let viewport_rect = context.logical_rect_to_viewport(&data.drawn_bounds);
			let image_draw_params = gelatin::glium::DrawParameters {
				viewport: Some(viewport_rect),
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
				let img_phys_siz;
				{
					let img_phys_w = image_display_width * context.dpi_scale_factor;
					let img_phys_h = image_display_height * context.dpi_scale_factor;
					img_phys_siz = LogicalVector::new(img_phys_w.ceil(), img_phys_h.ceil());
				}
				let corner_x =
					(img_pyhs_pos.x - img_phys_siz.vec.x * 0.5).floor() / context.dpi_scale_factor;
				let corner_y =
					(img_pyhs_pos.y - img_phys_siz.vec.y * 0.5).floor() / context.dpi_scale_factor;
				let adjusted_w = img_phys_siz.vec.x / context.dpi_scale_factor;
				let adjusted_h = img_phys_siz.vec.y / context.dpi_scale_factor;
				let transform = Matrix4::from_nonuniform_scale(adjusted_w, adjusted_h, 1.0);
				let transform =
					Matrix4::from_translation(Vector3::new(corner_x, corner_y, 0.0)) * transform;
				// Projection tranform
				let transform = projection_transform * transform;

				let sampler = texture
					.sampled()
					.minify_filter(
						gelatin::glium::uniforms::MinifySamplerFilter::LinearMipmapLinear,
					)
					.wrap_function(gelatin::glium::uniforms::SamplerWrapFunction::Clamp);
				let sampler = if data.img_texel_size >= 4f32 {
					sampler.magnify_filter(gelatin::glium::uniforms::MagnifySamplerFilter::Nearest)
				} else {
					sampler.magnify_filter(gelatin::glium::uniforms::MagnifySamplerFilter::Linear)
				};
				// building the uniforms
				let lod_level = ((1.0 / data.img_texel_size).log2().max(0.0) + 0.125).floor();
				let uniforms = uniform! {
					matrix: Into::<[[f32; 4]; 4]>::into(transform),
					bright_shade: data.bright_shade,
					tex: sampler,
					lod_level: lod_level,
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
		let borrowed = self.data.borrow();
		Ok(borrowed.next_update)
	}

	fn layout(&self, available_space: LogicalRect) {
		let mut borrowed = self.data.borrow_mut();
		borrowed.default_layout(available_space);
		borrowed.hover = borrowed.drawn_bounds.contains(borrowed.last_mouse_pos);
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
					borrowed.scaling = ScalingMode::Fixed;
					borrowed.update_scaling_buttons();
					borrowed.img_pos += delta;
					borrowed.render_validity.invalidate();
				}
				borrowed.last_mouse_pos = event.cursor_pos;
			}
			EventKind::MouseButton { state, button, .. } => match button {
				MouseButton::Left => {
					let mut borrowed = self.data.borrow_mut();
					if state == ElementState::Pressed && borrowed.hover {
						let now = Instant::now();
						let duration_since_last_click =
							now.duration_since(borrowed.last_click_time);
						borrowed.last_click_time = now;
						if duration_since_last_click < Duration::from_millis(250) {
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
					borrowed.render_validity.invalidate();
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
					borrowed.render_validity.invalidate();
				}
				_ => {}
			},
			EventKind::MouseScroll { delta } => {
				let mut borrowed = self.data.borrow_mut();
				let delta = delta.vec.y * 0.375;
				let delta = if delta > 0.0 { delta + 1.0 } else { 1.0 / (delta.abs() + 1.0) };

				let new_image_texel_size = (borrowed.img_texel_size * delta).max(0.0);

				borrowed.zoom_image(event.cursor_pos, new_image_texel_size);
			}
			EventKind::ReceivedCharacter(ch) => {
				let input_key = char_to_input_key(ch);
				self.handle_key_input(input_key.as_str(), event.modifiers);
			}
			EventKind::KeyInput { input } => {
				if let Some(key) = input.virtual_keycode {
					let input_key_str = virtual_keycode_to_string(key).to_lowercase();
					if !virtual_keycode_is_char(key) && input.state == ElementState::Pressed {
						self.handle_key_input(input_key_str.as_str(), event.modifiers);
					}
					// Panning is a special snowflake
					let mut borrowed = self.data.borrow_mut();
					if action_triggered(
						&borrowed.configuration,
						PAN_NAME,
						input_key_str.as_str(),
						event.modifiers,
					) {
						borrowed.panning = input.state == ElementState::Pressed;
					}
				}
			}
			EventKind::DroppedFile(ref path) => {
				let mut borrowed = self.data.borrow_mut();
				borrowed.playback_manager.request_load(LoadRequest::FilePath(path.clone()));
				borrowed.hover_state = HoverState::None;
				borrowed.render_validity.invalidate();
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
				borrowed.render_validity.invalidate();
			}
			EventKind::HoveredFileCancelled => {
				let mut borrowed = self.data.borrow_mut();
				match borrowed.hover_state.clone() {
					HoverState::None => {
						// Suprisingly this does happen sometimes, so let's just ignore this.
					}
					HoverState::ItemHovered { prev_path } => {
						borrowed.playback_manager.request_load(LoadRequest::FilePath(prev_path));
						borrowed.hover_state = HoverState::None;
					}
				}
				borrowed.render_validity.invalidate();
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

	fn set_valid_ref(&self, render_validity: RenderValidity) {
		self.data.borrow_mut().render_validity = render_validity;
	}
}
