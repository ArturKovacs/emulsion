use std::cell::{Ref, RefCell};
use std::path::PathBuf;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gelatin::cgmath::{Matrix4, Vector2, Vector3};
use gelatin::glium::glutin::event::{ElementState, ModifiersState, MouseButton};
use gelatin::glium::uniforms::{MinifySamplerFilter, SamplerWrapFunction};
use gelatin::glium::{
	program, uniform, uniforms::MagnifySamplerFilter, Display, Frame, Program, Surface,
};

use gelatin::add_common_widget_functions;
use gelatin::misc::{Alignment, Length, LogicalRect, LogicalVector, WidgetPlacement};
use gelatin::window::{RenderValidity, Window};
use gelatin::NextUpdate;
use gelatin::{
	application::request_exit, DrawContext, Event, EventKind, Widget, WidgetData, WidgetError,
};

use crate::input_handling::*;
use crate::shaders;
use crate::utils::{virtual_keycode_is_char, virtual_keycode_to_string};
use crate::{
	clipboard_handler::ClipboardHandler,
	configuration::{Antialias, Cache, Configuration},
	image_cache::{image_loader::Orientation, AnimationFrameTexture},
	playback_manager::*,
};

use super::{bottom_bar::BottomBar, copy_notification::CopyNotifications, help_screen::HelpScreen};

const MIN_ZOOM_FACTOR: f32 = 0.0001;
const MAX_ZOOM_FACTOR: f32 = 10000.0;
const AA_TEXEL_SIZE_THRESHOLD: f32 = 4f32;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ScalingMode {
	Fixed,
	FitStretch,
	FitMin,
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub enum MovementDir {
	None,
	Positive,
	Negative,
}

impl MovementDir {
	fn moving(self) -> bool {
		!matches!(self, MovementDir::None)
	}
}

#[derive(Debug, Clone)]
enum HoverState {
	None,
	ItemHovered { prev_path: PathBuf },
}

fn orientation_to_matrix(orientation: Orientation) -> Matrix4<f32> {
	#[rustfmt::skip]
	let result = match orientation {
		Orientation::Deg0 => Matrix4::from_scale(1.0),
		Orientation::Deg0HorFlip => Matrix4::new(
			-1.0, 0.0, 0.0, 0.0,
			0.0, 1.0, 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0
		),
		Orientation::Deg180 => Matrix4::new(
			-1.0, 0.0, 0.0, 0.0,
			0.0, -1.0, 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0
		),
		Orientation::Deg180HorFlip => Matrix4::new(
			1.0, 0.0, 0.0, 0.0,
			0.0, -1.0, 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0
		),
		Orientation::Deg90 => Matrix4::new(
			0.0, -1.0, 0.0, 0.0,
			1.0, 0.0, 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0
		),
		Orientation::Deg90VerFlip => Matrix4::new(
			0.0, -1.0, 0.0, 0.0,
			-1.0, 0.0, 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0
		),
		Orientation::Deg270 => Matrix4::new(
			0.0, 1.0, 0.0, 0.0,
			-1.0, 0.0, 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0
		),
		Orientation::Deg270VerFlip => Matrix4::new(
			0.0, 1.0, 0.0, 0.0,
			1.0, 0.0, 0.0, 0.0,
			0.0, 0.0, 1.0, 0.0,
			0.0, 0.0, 0.0, 1.0
		),
	};
	result
}

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
	// It's an option to allow manual destruction.
	clipboard_handler: Option<ClipboardHandler>,
	clipboard_request_was_pending: bool,

	program: Program,
	bright_shade: f32,
	/// Size of an image texel in physical display pixels
	img_texel_size: f32,
	scaling: ScalingMode,
	img_pos: LogicalVector,
	antialiasing: Antialias,

	hor_pan_input: MovementDir,
	ver_pan_input: MovementDir,
	zoom_input: MovementDir,
	/// The velocity of horizontal panning
	hor_pan_vel: f32,
	/// The velocity of vertical panning
	ver_pan_vel: f32,
	/// The velocity of zooming
	zoom_vel: f32,

	last_click_time: Instant,
	last_mouse_pos: LogicalVector,
	panning_2d: bool,
	panning_vert: bool,
	panning_hor: bool,
	hover_state: HoverState,

	first_draw: bool,
	last_cam_move_time: Instant,
	next_update: NextUpdate,
	bottom_bar: Rc<BottomBar>,
	left_to_pan_hint: Rc<HelpScreen>,
	copy_notifications: CopyNotifications,
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
	fn fit_image_to_panel(&mut self, dpi_scale: f32, stretch: bool) {
		let size = self.drawn_bounds.size.vec;
		if let Some(texture) = self.get_texture() {
			let panel_aspect = size.x / size.y;
			let (img_phys_w, img_pyhs_h) = {
				let (w, h) = texture.oriented_dimensions();
				(w as f32, h as f32)
			};
			let img_aspect = img_phys_w / img_pyhs_h;

			let texel_size_to_fit_width = size.x / img_phys_w;
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

	fn zoom_image(&mut self, anchor: LogicalVector, mut delta: f32) {
		delta = if delta > 0.0 { delta + 1.0 } else { 1.0 / (delta.abs() + 1.0) };
		let mut image_texel_size = (self.img_texel_size * delta).max(0.0);
		if (image_texel_size - 1.0).abs() < 0.01 {
			image_texel_size = 1.0;
		} else if image_texel_size < MIN_ZOOM_FACTOR {
			image_texel_size = MIN_ZOOM_FACTOR;
		} else if image_texel_size > MAX_ZOOM_FACTOR {
			image_texel_size = MAX_ZOOM_FACTOR;
		}
		self.img_pos = (image_texel_size / self.img_texel_size) * (self.img_pos - anchor) + anchor;
		self.img_texel_size = image_texel_size;
		self.scaling = ScalingMode::Fixed;
		self.update_scaling_buttons();
		self.render_validity.invalidate();
	}

	fn update_image_transform(&mut self, dpi_scale: f32) {
		match self.scaling {
			ScalingMode::Fixed => {
				let center_offset = (self.drawn_bounds.size - self.prev_draw_size) * 0.5f32;
				self.img_pos += center_offset;
				self.apply_img_bounds(dpi_scale);
			}
			ScalingMode::FitStretch => {
				self.fit_image_to_panel(dpi_scale, true);
			}
			ScalingMode::FitMin => {
				self.fit_image_to_panel(dpi_scale, false);
			}
		}
		self.prev_draw_size = self.drawn_bounds.size;
	}

	fn apply_camera_movement(&mut self, dpi_scale: f32) {
		fn animate_value(v: &mut f32, dir: f32, dt: f32, next_update: &mut NextUpdate) {
			#[allow(clippy::float_cmp)]
			if v.signum() != dir {
				*v = 0.0;
			}
			*v += dir * dt * (2.0 / (v.abs() + 1.0));
			*next_update = NextUpdate::Soonest;
		}

		let now = Instant::now();
		let dt_sec = now.duration_since(self.last_cam_move_time).as_secs_f32();
		self.last_cam_move_time = now;

		match self.hor_pan_input {
			MovementDir::None => self.hor_pan_vel = 0.0,
			MovementDir::Positive => {
				animate_value(&mut self.hor_pan_vel, 1.0, dt_sec, &mut self.next_update)
			}
			MovementDir::Negative => {
				animate_value(&mut self.hor_pan_vel, -1.0, dt_sec, &mut self.next_update)
			}
		}
		match self.ver_pan_input {
			MovementDir::None => self.ver_pan_vel = 0.0,
			MovementDir::Positive => {
				animate_value(&mut self.ver_pan_vel, 1.0, dt_sec, &mut self.next_update)
			}
			MovementDir::Negative => {
				animate_value(&mut self.ver_pan_vel, -1.0, dt_sec, &mut self.next_update)
			}
		}
		match self.zoom_input {
			MovementDir::None => self.zoom_vel = 0.0,
			MovementDir::Positive => {
				animate_value(&mut self.zoom_vel, 1.0, dt_sec, &mut self.next_update)
			}
			MovementDir::Negative => {
				animate_value(&mut self.zoom_vel, -1.0, dt_sec, &mut self.next_update)
			}
		}

		if self.zoom_input.moving() {
			let bounds_size = self.drawn_bounds.size.vec;
			let anchor = LogicalVector::new(bounds_size.x * 0.5, bounds_size.y * 0.5);
			self.zoom_image(anchor, self.zoom_vel * dt_sec);
		}
		if self.hor_pan_input.moving() || self.ver_pan_input.moving() {
			let panning_speed = 400.0 * dpi_scale;
			let pos_delta = Vector2::new(self.hor_pan_vel, self.ver_pan_vel) * dt_sec;
			self.scaling = ScalingMode::Fixed;
			self.update_scaling_buttons();
			self.img_pos.vec += panning_speed * pos_delta;
		}
	}

	fn camera_movement_will_start(&mut self) {
		// If there hasn't been any movement in a while, then reset the last update time
		// to avoid large jumps at the beggining of a move when the delta would be large.
		if !self.hor_pan_input.moving() && !self.ver_pan_input.moving() && !self.zoom_input.moving()
		{
			self.last_cam_move_time = Instant::now();
		}
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
		window.set_title(title);
	}

	fn get_texture(&self) -> Option<AnimationFrameTexture> {
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

	pub fn toggle_antialias(&mut self) {
		let aa = match self.antialiasing {
			Antialias::Auto if self.img_texel_size < AA_TEXEL_SIZE_THRESHOLD => Antialias::Never,
			Antialias::Auto | Antialias::Never => Antialias::Always,
			Antialias::Always => Antialias::Never,
		};
		self.antialiasing = aa;
		self.cache.lock().unwrap().image.antialiasing = aa;
		self.render_validity.invalidate();
	}

	pub fn set_automatic_antialias(&mut self) {
		self.antialiasing = Antialias::Auto;
		self.cache.lock().unwrap().image.antialiasing = Antialias::Auto;
		self.render_validity.invalidate();
	}

	/// Ensures that the image is within the widget, or at least touches an edge of the widget
	fn apply_img_bounds(&mut self, dpi_scale: f32) {
		if let Some(texture) = self.get_texture() {
			let (img_phys_w, img_phys_h) = {
				let (w, h) = texture.oriented_dimensions();
				(w as f32 * self.img_texel_size, h as f32 * self.img_texel_size)
			};
			let img_w = img_phys_w / dpi_scale;
			let img_h = img_phys_h / dpi_scale;

			let widget_size = self.drawn_bounds.size.vec;
			let img_pos = self.img_pos.vec;

			if img_pos.x < -img_w / 2.0 {
				self.img_pos.vec.x = -img_w / 2.0;
			}
			if img_pos.y < -img_h / 2.0 {
				self.img_pos.vec.y = -img_h / 2.0;
			}

			if img_pos.x > widget_size.x + img_w / 2.0 {
				self.img_pos.vec.x = (widget_size.x + img_w / 2.0).ceil();
			}
			if img_pos.y > widget_size.y + img_h / 2.0 {
				self.img_pos.vec.y = (widget_size.y + img_h / 2.0).ceil();
			}
		}
	}

	fn update_scaling_buttons(&mut self) {
		self.bottom_bar.update_scaling_buttons(self.scaling, self.img_texel_size);
	}
}

pub struct PictureWidget {
	data: RefCell<PictureWidgetData>,
}
impl PictureWidget {
	pub fn new(
		display: &Display,
		window: &Rc<Window>,
		bottom_bar: Rc<BottomBar>,
		left_to_pan_hint: Rc<HelpScreen>,
		copy_notifications: CopyNotifications,
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

		let antialiasing = configuration
			.borrow()
			.image
			.as_ref()
			.and_then(|s| s.antialiasing.clone())
			.unwrap_or_else(|| "auto".into());

		let antialiasing = match antialiasing.as_str() {
			"auto" => Antialias::Auto,
			"always" => Antialias::Always,
			"never" => Antialias::Never,
			"previous" => cache.lock().unwrap().image.antialiasing,
			val => {
				eprintln!("Illegal configuration value {:?} for antialiasing!", val);
				eprintln!(r#"Allowed values are "auto", "always", "never" and "previous"."#);
				Antialias::default()
			}
		};

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
			clipboard_handler: Some(ClipboardHandler::new()),
			clipboard_request_was_pending: false,
			render_validity: Default::default(),

			program,
			bright_shade: 0.95,
			img_texel_size: 0.0,
			scaling,
			img_pos: Default::default(),
			antialiasing,
			hor_pan_input: MovementDir::None,
			ver_pan_input: MovementDir::None,
			zoom_input: MovementDir::None,
			hor_pan_vel: 0.0,
			ver_pan_vel: 0.0,
			zoom_vel: 0.0,
			last_click_time: Instant::now() - Duration::from_secs(10),
			last_mouse_pos: Default::default(),
			panning_2d: false,
			panning_vert: false,
			panning_hor: false,
			hover_state: HoverState::None,
			last_cam_move_time: Instant::now(),
			first_draw: true,
			next_update: NextUpdate::Latest,
			bottom_bar,
			left_to_pan_hint,
			copy_notifications,
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
				borrowed.bottom_bar.set_visible_if_should_show(!fullscreen);
			}
		}
		if triggered!(ESCAPE_NAME) {
			if let Some(window) = borrowed.window.upgrade() {
				if window.fullscreen() {
					window.set_fullscreen(false);
					borrowed.bottom_bar.set_visible_if_should_show(true);
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
		if triggered!(TOGGLE_ANTIALIAS_NAME) {
			borrowed.toggle_antialias();
		}
		if triggered!(SET_AUTOMATIC_ANTIALIAS_NAME) {
			borrowed.set_automatic_antialias();
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
			if let Some(path) = borrowed.playback_manager.shown_file_path() {
				if let Err(e) = trash::delete(&path) {
					eprintln!("Error while moving file '{:?}' to trash: {:?}", path, e);
				}
				if let Err(e) = borrowed.playback_manager.update_directory() {
					eprintln!("Error while updating directory {:?}", e);
				}
				borrowed.render_validity.invalidate();
			}
		}
		if triggered!(IMG_COPY_NAME) {
			if let Some(path) = borrowed.playback_manager.shown_file_path().clone() {
				let request_started;
				if let Some(clipboard_handler) = &mut borrowed.clipboard_handler {
					request_started = true;
					clipboard_handler.request_copy(path);
					borrowed.copy_notifications.set_started();
				} else {
					request_started = false;
				}
				if request_started {
					borrowed.clipboard_request_was_pending = true;
				}
			}
		}
		if let Some(img_path) = borrowed.playback_manager.shown_file_path() {
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
		let now = Instant::now();
		let prev_texture = data.playback_manager.image_texture();
		data.next_update = data.playback_manager.update_image(window);
		let new_texture = data.playback_manager.image_texture();
		let curr_file_index = data.playback_manager.current_file_index();
		let curr_dir_len = data.playback_manager.current_dir_len();
		if let (Some(curr_file_index), Some(curr_dir_len)) = (curr_file_index, curr_dir_len) {
			data.bottom_bar.slider.set_steps(curr_dir_len as u32, curr_file_index as u32);
		}
		//data.slider.set_step_bg(data.playback_manager.cached_from_dir());
		let playback_state = data.playback_manager.playback_state();
		data.set_window_title_filename(
			window,
			playback_state,
			data.playback_manager.shown_file_path(),
		);
		if prev_texture.is_none() != new_texture.is_none() {
			data.render_validity.invalidate();
		} else if let (Some(prev_tex), Some(new_tex)) = (prev_texture, new_texture) {
			if !Rc::ptr_eq(&prev_tex.tex_grid, &new_tex.tex_grid) {
				data.render_validity.invalidate();
			}
		}
		if let Some(clipboard_handler) = &data.clipboard_handler {
			let clipboard_result = clipboard_handler.try_get_result();
			let request_pending = clipboard_result.is_none();
			if data.clipboard_request_was_pending != request_pending {
				match clipboard_result {
					Some(succeeded) => data.copy_notifications.set_finished(succeeded),
					None => data.copy_notifications.set_started(),
				}
				data.clipboard_request_was_pending = request_pending;
			} else if request_pending {
				let next_update = now + Duration::from_millis(100);
				data.next_update = data.next_update.aggregate(NextUpdate::WaitUntil(next_update));
			}
		}
		if data.zoom_input.moving() || data.hor_pan_input.moving() || data.ver_pan_input.moving() {
			data.render_validity.invalidate();
			data.next_update = NextUpdate::Soonest;
		}
		let next_copy_noti_update = data.copy_notifications.update();
		data.next_update = data.next_update.aggregate(next_copy_noti_update);
		data.next_update
	}

	fn draw(&self, target: &mut Frame, context: &DrawContext) -> Result<NextUpdate, WidgetError> {
		let texture;
		{
			let mut data = self.data.borrow_mut();
			if !data.visible {
				return Ok(data.next_update);
			}
			data.update_image_transform(context.dpi_scale_factor);
			data.apply_camera_movement(context.dpi_scale_factor);
			texture = data.get_texture();
		}
		if let Some(texture) = texture {
			let data = self.data.borrow();
			draw_tex_grid(data, target, context, texture);
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
				if borrowed.panning_2d || borrowed.panning_hor || borrowed.panning_vert {
					let mut delta = event.cursor_pos - borrowed.last_mouse_pos;
					if !borrowed.panning_2d {
						if !borrowed.panning_hor {
							// only vertical panning
							delta.vec.x = 0.0;
						}
						if !borrowed.panning_vert {
							// only horzontal panning
							delta.vec.y = 0.0;
						}
					}
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
					if state == ElementState::Pressed {
						if borrowed.hover {
							borrowed.click = true;
							borrowed.panning_2d = true
						}
					} else {
						borrowed.panning_2d = false;
						borrowed.click = false;
						if borrowed.hover {
							let now = Instant::now();
							let duration_since_last_click =
								now.duration_since(borrowed.last_click_time);
							borrowed.last_click_time = now;
							if duration_since_last_click < Duration::from_millis(250) {
								match borrowed.window.upgrade() {
									Some(window) => {
										let fullscreen = !window.fullscreen();
										window.set_fullscreen(fullscreen);
										borrowed.bottom_bar.set_visible_if_should_show(!fullscreen);
									}
									None => unreachable!(),
								}
							}
						}
					}
					borrowed.render_validity.invalidate();
				}
				MouseButton::Right => {
					let borrowed = self.data.borrow();
					let pressed = state == ElementState::Pressed;
					borrowed.left_to_pan_hint.set_visible(pressed);
				}
				_ => {}
			},
			EventKind::MouseScroll { delta } => {
				let mut borrowed = self.data.borrow_mut();
				let delta = delta.vec.y * 0.375;
				borrowed.zoom_image(event.cursor_pos, delta);
			}
			EventKind::ReceivedCharacter(ch) => {
				//println!("Got char {}", ch);
				// When the control key is held down, this character is going to be the keycode
				// of an ascii control character
				// See https://en.wikipedia.org/wiki/Caret_notation
				if !event.modifiers.ctrl() {
					let input_key = char_to_input_key(ch);
					//println!("triggering for char {}, input str: {}", ch, input_key);
					self.handle_key_input(input_key.as_str(), event.modifiers);
				}
			}
			EventKind::KeyInput { input } => {
				if let Some(key) = input.virtual_keycode {
					//println!("Got input for {:?}", key);
					let input_key_str = virtual_keycode_to_string(key).to_lowercase();
					let printable = !event.modifiers.ctrl() && virtual_keycode_is_char(key);
					if !printable && input.state == ElementState::Pressed {
						//println!("Triggering for input {:?}", key);
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
						borrowed.panning_2d = input.state == ElementState::Pressed;
					}
					if action_triggered(
						&borrowed.configuration,
						PAN_VERT_NAME,
						input_key_str.as_str(),
						event.modifiers,
					) {
						borrowed.panning_vert = input.state == ElementState::Pressed;
					}
					if action_triggered(
						&borrowed.configuration,
						PAN_HOR_NAME,
						input_key_str.as_str(),
						event.modifiers,
					) {
						borrowed.panning_hor = input.state == ElementState::Pressed;
					}

					let pressed = input.state == ElementState::Pressed;

					macro_rules! movement_trigger {
						($input:expr, $vel:expr, $name:expr, $dir:expr) => {
							if action_triggered(
								&borrowed.configuration,
								$name,
								input_key_str.as_str(),
								event.modifiers,
							) {
								if $input == $dir && !pressed {
									$input = MovementDir::None;
									$vel = 0.0;
								}
								if $input != $dir && pressed {
									borrowed.camera_movement_will_start();
									$input = $dir;
								}
							}
						};
					}

					movement_trigger!(
						borrowed.zoom_input,
						borrowed.zoom_vel,
						ZOOM_IN_NAME,
						MovementDir::Positive
					);
					movement_trigger!(
						borrowed.zoom_input,
						borrowed.zoom_vel,
						ZOOM_OUT_NAME,
						MovementDir::Negative
					);

					movement_trigger!(
						borrowed.hor_pan_input,
						borrowed.hor_pan_vel,
						PAN_LEFT_NAME,
						MovementDir::Positive
					);
					movement_trigger!(
						borrowed.hor_pan_input,
						borrowed.hor_pan_vel,
						PAN_RIGHT_NAME,
						MovementDir::Negative
					);

					movement_trigger!(
						borrowed.ver_pan_input,
						borrowed.ver_pan_vel,
						PAN_UP_NAME,
						MovementDir::Positive
					);
					movement_trigger!(
						borrowed.ver_pan_input,
						borrowed.ver_pan_vel,
						PAN_DOWN_NAME,
						MovementDir::Negative
					);
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
						let curr_path = borrowed
							.playback_manager
							.shown_file_path()
							.clone()
							.unwrap_or_else(PathBuf::new);
						borrowed.hover_state = HoverState::ItemHovered { prev_path: curr_path };
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
			EventKind::Focused(focused) => {
				if focused {
					let mut borrowed = self.data.borrow_mut();
					if let Err(e) = borrowed.playback_manager.update_directory() {
						eprintln!("{}", e);
					}
					borrowed.render_validity.invalidate();
				}
			}
			EventKind::CloseRequested => {
				let mut borrowed = self.data.borrow_mut();
				// Just let it drop.
				borrowed.clipboard_handler.take();
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

impl Drop for PictureWidget {
	fn drop(&mut self) {
		// This doesn't work. Would be nice to fix at some point. I think I managed to create a
		// circular reference with my `Rc`s
		//println!("Called drop for the picture widget.");
	}
}

fn draw_tex_grid(
	data: Ref<PictureWidgetData>,
	target: &mut Frame,
	context: &DrawContext,
	texture: AnimationFrameTexture,
) {
	let size = data.drawn_bounds.size.vec;
	let projection_transform = gelatin::cgmath::ortho(0.0, size.x, size.y, 0.0, -1.0, 1.0);

	let viewport_rect = context.logical_rect_to_viewport(&data.drawn_bounds);
	let image_draw_params =
		gelatin::glium::DrawParameters { viewport: Some(viewport_rect), ..Default::default() };

	let img_phys_w = texture.w as f32;
	let img_phys_h = texture.h as f32;
	let img_height_over_width = img_phys_h / img_phys_w;
	let image_display_width = data.img_texel_size * img_phys_w / context.dpi_scale_factor;
	let image_display_height = image_display_width * img_height_over_width;
	// Model tranform
	let img_pyhs_pos = data.img_pos.vec * context.dpi_scale_factor;
	let img_phys_siz = {
		let img_phys_w = image_display_width * context.dpi_scale_factor;
		let img_phys_h = image_display_height * context.dpi_scale_factor;
		LogicalVector::new(img_phys_w.ceil(), img_phys_h.ceil())
	};
	let img_logical_corner_x =
		(img_pyhs_pos.x - img_phys_siz.vec.x * 0.5).ceil() / context.dpi_scale_factor;
	let img_logical_corner_y =
		(img_pyhs_pos.y - img_phys_siz.vec.y * 0.5).ceil() / context.dpi_scale_factor;

	// This is the display width of the image in logical pixel units
	let img_adjusted_w = img_phys_siz.vec.x / context.dpi_scale_factor;
	// This is the display height of the image in logical pixel units
	let img_adjusted_h = img_phys_siz.vec.y / context.dpi_scale_factor;
	let img_scaling = Matrix4::from_nonuniform_scale(img_adjusted_w, img_adjusted_h, 1.0);
	let orientation;
	{
		let to_center = Matrix4::from_translation(Vector3::new(
			-0.5 * img_adjusted_w,
			-0.5 * img_adjusted_h,
			0.0,
		));
		let orient = orientation_to_matrix(texture.orientation);
		let to_corner = Matrix4::from_translation(Vector3::new(
			0.5 * img_adjusted_w,
			0.5 * img_adjusted_h,
			0.0,
		));
		orientation = to_corner * orient * to_center;
	}
	let img_translation =
		Matrix4::from_translation(Vector3::new(img_logical_corner_x, img_logical_corner_y, 0.0));

	// let img_logical_w = img_w / context.dpi_scale_factor;
	// let img_logical_h = img_h / context.dpi_scale_factor;
	let cell_phy_step = texture.cell_step_size;
	for cell_tex in texture.tex_grid.iter() {
		let (cell_phys_w, cell_phys_h) = cell_tex.tex.dimensions();

		let cell_phy_offset_x = cell_phy_step * cell_tex.col;
		let cell_phy_offset_y = cell_phy_step * cell_tex.row;
		// let cell_logical_offset_x = cell_phy_offset_x as f32 / context.dpi_scale_factor;
		// let cell_logical_offset_y = cell_phy_offset_y as f32 / context.dpi_scale_factor;

		// The grid is constructed so that it is exactly of size (1, 1) and is located at (0, 0)
		// This allows to leave most of the image transformation logic unchanged.
		let cell_scaling = Matrix4::from_nonuniform_scale(
			cell_phys_w as f32 / img_phys_w,
			cell_phys_h as f32 / img_phys_h,
			1.0,
		);
		let cell_translation = Matrix4::from_translation(Vector3::new(
			cell_phy_offset_x as f32 / img_phys_w,
			cell_phy_offset_y as f32 / img_phys_h,
			0.0,
		));

		let transform =
			img_translation * orientation * img_scaling * cell_translation * cell_scaling;
		// Projection tranform
		let transform = projection_transform * transform;

		let sampler = cell_tex
			.tex
			.sampled()
			.minify_filter(MinifySamplerFilter::LinearMipmapLinear)
			.wrap_function(SamplerWrapFunction::Clamp);

		// let filter = match data.antialiasing {
		// 	Antialias::Auto if data.img_texel_size < AA_TEXEL_SIZE_THRESHOLD => {
		// 		MagnifySamplerFilter::Linear
		// 	}
		// 	Antialias::Auto | Antialias::Never => MagnifySamplerFilter::Nearest,
		// 	Antialias::Always => MagnifySamplerFilter::Linear,
		// };
		let filter = MagnifySamplerFilter::Linear;
		let sampler = sampler.magnify_filter(filter);

		let sampler_nearest = cell_tex
			.tex
			.sampled()
			.minify_filter(MinifySamplerFilter::Nearest)
			.magnify_filter(MagnifySamplerFilter::Nearest)
			.wrap_function(SamplerWrapFunction::Clamp);

		// building the uniforms
		let height = cell_tex.tex.get_height().unwrap_or(1);
		let texel_size = [1.0 / cell_tex.tex.get_width() as f32, 1.0 / height as f32];
		let lod_level = ((1.0 / data.img_texel_size).log2().max(0.0) + 0.125).floor();
		let uniforms = uniform! {
			matrix: Into::<[[f32; 4]; 4]>::into(transform),
			bright_shade: data.bright_shade,
			tex: sampler,
			tex_nearest: sampler_nearest,
			lod_level: lod_level,
			texel_size: texel_size,
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
