#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate error_chain;

use std::cell::{Cell, RefCell};
use std::f32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use directories::ProjectDirs;
use lazy_static::lazy_static;
#[cfg(feature = "networking")]
use serde_derive::Deserialize;

use gelatin::glium::glutin::{
	dpi::{PhysicalPosition, PhysicalSize},
	event::WindowEvent,
	window::Icon,
};
use gelatin::{
	application::*,
	button::*,
	image,
	label::*,
	line_layout_container::*,
	misc::*,
	picture::*,
	slider::*,
	window::{Window, WindowDescriptorBuilder},
	NextUpdate, Widget,
};

use crate::configuration::{Cache, Configuration};
use crate::help_screen::*;
use crate::picture_widget::*;
#[cfg(feature = "networking")]
use crate::version::Version;

mod configuration;
mod handle_panic;
mod help_screen;
mod image_cache;
mod picture_widget;
mod playback_manager;
mod shaders;
mod utils;
#[cfg(feature = "networking")]
mod version;

lazy_static! {
	pub static ref PROJECT_DIRS: Option<ProjectDirs> = ProjectDirs::from("", "", "emulsion");
}

static NEW_VERSION: &[u8] = include_bytes!("../resource/new-version-available.png");
static NEW_VERSION_LIGHT: &[u8] = include_bytes!("../resource/new-version-available-light.png");
static VISIT_SITE: &[u8] = include_bytes!("../resource/visit-site.png");
static USAGE: &[u8] = include_bytes!("../resource/usage.png");
static MOON: &[u8] = include_bytes!("../resource/moon.png");
static LIGHT: &[u8] = include_bytes!("../resource/light.png");
static QUESTION_BUTTON: &[u8] = include_bytes!("../resource/question_button.png");
static QUESTION_BUTTON_LIGHT: &[u8] = include_bytes!("../resource/question_button_light.png");
static QUESTION_NOTI: &[u8] = include_bytes!("../resource/question-noti.png");
static QUESTION_LIGHT_NOTI: &[u8] = include_bytes!("../resource/question-light-noti.png");

// ========================================================
// Not-so glorious main function
// ========================================================
fn main() {
	std::panic::set_hook(Box::new(handle_panic::handle_panic));

	// Load configuration and cache files
	let (config_path, cache_path) = get_config_and_cache_paths();

	let cache = Cache::load(&cache_path);
	let config = Configuration::load(&config_path);

	let first_launch = cache.is_err();
	let cache = Arc::new(Mutex::new(cache.unwrap_or_default()));
	let config = Rc::new(RefCell::new(config.unwrap_or_default()));

	let mut application = Application::new();
	let window: Rc<Window> = {
		let cache = cache.lock().unwrap();

		let window_desc = WindowDescriptorBuilder::default()
			.icon(Some(make_icon()))
			.size(PhysicalSize::new(cache.window.win_w, cache.window.win_h))
			.position(Some(PhysicalPosition::new(cache.window.win_x, cache.window.win_y)))
			.build()
			.unwrap();
		Window::new(&mut application, window_desc)
	};
	{
		let cache = cache.clone();

		window.add_global_event_handler(move |event| match event {
			WindowEvent::Resized(new_size) => {
				let mut cache = cache.lock().unwrap();
				cache.window.win_w = new_size.width;
				cache.window.win_h = new_size.height;
			}
			WindowEvent::Moved(new_pos) => {
				let mut cache = cache.lock().unwrap();
				cache.window.win_x = new_pos.x;
				cache.window.win_y = new_pos.y;
			}
			_ => (),
		});
	}

	let update_label_image = Rc::new(Picture::from_encoded_bytes(NEW_VERSION));
	let update_label_image_light = Rc::new(Picture::from_encoded_bytes(NEW_VERSION_LIGHT));
	let update_label = make_update_label();

	let update_notification = make_update_notification(update_label.clone());

	let usage_img = Picture::from_encoded_bytes(USAGE);
	let help_screen = Rc::new(HelpScreen::new(usage_img));

	let question = Rc::new(Picture::from_encoded_bytes(QUESTION_BUTTON));
	let question_light = Rc::new(Picture::from_encoded_bytes(QUESTION_BUTTON_LIGHT));
	let question_noti = Rc::new(Picture::from_encoded_bytes(QUESTION_NOTI));
	let question_light_noti = Rc::new(Picture::from_encoded_bytes(QUESTION_LIGHT_NOTI));
	let moon_img = Rc::new(Picture::from_encoded_bytes(MOON));
	let light_img = Rc::new(Picture::from_encoded_bytes(LIGHT));

	let theme_button = make_theme_button();
	let help_button = make_help_button();
	let slider = make_slider();

	let bottom_container = make_bottom_container();
	bottom_container.add_child(theme_button.clone());
	bottom_container.add_child(slider.clone());
	bottom_container.add_child(help_button.clone());

	let picture_widget =
		make_picture_widget(&window, slider.clone(), bottom_container.clone(), config.clone());

	let picture_area_container = make_picture_area_container();
	picture_area_container.add_child(picture_widget.clone());
	picture_area_container.add_child(help_screen.clone());
	picture_area_container.add_child(update_notification.clone());

	let root_container = make_root_container();
	root_container.add_child(picture_area_container);
	root_container.add_child(bottom_container.clone());

	let update_available = Arc::new(AtomicBool::new(false));
	let update_check_done = Arc::new(AtomicBool::new(false));
	let light_theme = Rc::new(Cell::new(!cache.lock().unwrap().window.dark));

	let set_theme = {
		let theme_button = theme_button.clone();
		let help_button = help_button.clone();
		let update_label = update_label;
		let picture_widget = picture_widget.clone();
		let bottom_container = bottom_container;
		let update_notification = update_notification.clone();
		let slider = slider.clone();
		let window = window.clone();
		let light_theme = light_theme.clone();
		let update_available = update_available.clone();

		Rc::new(move || {
			if light_theme.get() {
				picture_widget.set_bright_shade(0.96);
				bottom_container.set_bg_color([1.0, 1.0, 1.0, 1.0]);
				slider.set_shadow_color([0.0, 0.0, 0.0]);
				window.set_bg_color([0.85, 0.85, 0.85, 1.0]);
				theme_button.set_icon(Some(moon_img.clone()));
				update_notification.set_bg_color([0.06, 0.06, 0.06, 1.0]);
				update_label.set_icon(Some(update_label_image_light.clone()));
				if update_available.load(Ordering::SeqCst) {
					help_button.set_icon(Some(question_noti.clone()));
				} else {
					help_button.set_icon(Some(question.clone()));
				}
			} else {
				picture_widget.set_bright_shade(0.11);
				bottom_container.set_bg_color([0.08, 0.08, 0.08, 1.0]);
				slider.set_shadow_color([0.0, 0.0, 0.0]);
				window.set_bg_color([0.03, 0.03, 0.03, 1.0]);
				theme_button.set_icon(Some(light_img.clone()));
				update_notification.set_bg_color([0.85, 0.85, 0.85, 1.0]);
				update_label.set_icon(Some(update_label_image.clone()));
				if update_available.load(Ordering::SeqCst) {
					help_button.set_icon(Some(question_light_noti.clone()));
				} else {
					help_button.set_icon(Some(question_light.clone()));
				}
			}
		})
	};
	set_theme();
	{
		let cache = cache.clone();
		let set_theme = set_theme.clone();
		theme_button.set_on_click(move || {
			light_theme.set(!light_theme.get());
			cache.lock().unwrap().window.dark = !light_theme.get();
			set_theme();
		});
	}
	{
		let slider_clone = slider.clone();
		slider.set_on_value_change(move || {
			picture_widget.jump_to_index(slider_clone.value());
		});
	}
	let help_visible = Cell::new(first_launch);
	help_screen.set_visible(help_visible.get());
	update_notification.set_visible(help_visible.get() && update_available.load(Ordering::SeqCst));
	{
		let update_available = update_available.clone();
		let help_screen = help_screen.clone();
		let update_notification = update_notification.clone();

		help_button.set_on_click(move || {
			help_visible.set(!help_visible.get());
			help_screen.set_visible(help_visible.get());
			update_notification
				.set_visible(help_visible.get() && update_available.load(Ordering::SeqCst));
		});
	}

	window.set_root(root_container);

	let check_updates_enabled = match &config.borrow().updates {
		Some(u) if !u.check_updates => false,
		_ => true,
	};

	let update_checker_join_handle = {
		let updates = &mut cache.lock().unwrap().updates;
		let cache = cache.clone();
		let update_available = update_available.clone();
		let update_check_done = update_check_done.clone();

		if check_updates_enabled && updates.update_check_needed() {
			// kick off a thread that will check for an update in the background
			Some(std::thread::spawn(move || {
				let has_update = check_for_updates();
				update_available.store(has_update, Ordering::SeqCst);
				update_check_done.store(true, Ordering::SeqCst);
				if !has_update {
					cache.lock().unwrap().updates.set_update_check_time();
				}
			}))
		} else {
			None
		}
	};

	let mut nothing_to_do = false;
	application.add_global_event_handler(move |_| {
		if nothing_to_do {
			return NextUpdate::Latest;
		}
		if update_check_done.load(Ordering::SeqCst) {
			nothing_to_do = true;
			set_theme();
			if help_screen.visible() && update_available.load(Ordering::SeqCst) {
				update_notification.set_visible(true);
			}
		}
		NextUpdate::WaitUntil(Instant::now() + Duration::from_secs(1))
	});

	application.set_at_exit(Some(move || {
		cache.lock().unwrap().save(cache_path).unwrap();
		if let Some(h) = update_checker_join_handle {
			h.join().unwrap();
		}
	}));
	application.start_event_loop();
}
// ========================================================

fn make_icon() -> Icon {
	let img = image::load_from_memory(include_bytes!("../resource/emulsion48.png")).unwrap();
	let rgba = img.into_rgba();
	let (w, h) = rgba.dimensions();
	Icon::from_rgba(rgba.into_raw(), w, h).unwrap()
}

fn make_root_container() -> Rc<VerticalLayoutContainer> {
	let container = Rc::new(VerticalLayoutContainer::new());
	container.set_margin_all(0.0);
	container.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
	container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
	container
}

fn make_picture_area_container() -> Rc<VerticalLayoutContainer> {
	let picture_area_container = Rc::new(VerticalLayoutContainer::new());
	picture_area_container.set_margin_all(0.0);
	picture_area_container.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
	picture_area_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
	picture_area_container
}

fn make_update_label() -> Rc<Label> {
	let update_label = Rc::new(Label::new());
	update_label.set_margin_top(4.0);
	update_label.set_margin_bottom(4.0);
	update_label.set_fixed_size(LogicalVector::new(200.0, 24.0));
	update_label.set_horizontal_align(Alignment::Center);
	update_label
}

fn make_update_notification(update_label: Rc<Label>) -> Rc<HorizontalLayoutContainer> {
	let container = Rc::new(HorizontalLayoutContainer::new());
	container.set_vertical_align(Alignment::End);
	container.set_horizontal_align(Alignment::Start);
	container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
	container.set_height(Length::Fixed(32.0));

	let update_button = Rc::new(Button::new());
	let button_image = Rc::new(Picture::from_encoded_bytes(VISIT_SITE));
	update_button.set_icon(Some(button_image));
	update_button.set_margin_top(4.0);
	update_button.set_margin_bottom(4.0);
	update_button.set_fixed_size(LogicalVector::new(100.0, 24.0));
	update_button.set_horizontal_align(Alignment::Center);
	update_button.set_on_click(|| {
		open::that("https://arturkovacs.github.io/emulsion-website/").unwrap();
	});

	container.add_child(update_label);
	container.add_child(update_button);
	container
}

fn make_theme_button() -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(5.0);
	button.set_margin_left(28.0);
	button.set_margin_right(4.0);
	button.set_height(Length::Fixed(24.0));
	button.set_width(Length::Fixed(24.0));
	button.set_horizontal_align(Alignment::Center);
	button
}

fn make_help_button() -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(5.0);
	button.set_margin_left(4.0);
	button.set_margin_right(28.0);
	button.set_height(Length::Fixed(24.0));
	button.set_width(Length::Fixed(24.0));
	button.set_horizontal_align(Alignment::Center);
	button
}

fn make_bottom_container() -> Rc<HorizontalLayoutContainer> {
	let container = Rc::new(HorizontalLayoutContainer::new());
	// container.set_margin_top(4.0);
	// container.set_margin_bottom(4.0);
	container.set_margin_left(0.0);
	container.set_margin_right(0.0);
	container.set_height(Length::Fixed(32.0));
	container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
	container
}

fn make_slider() -> Rc<Slider> {
	let slider = Rc::new(Slider::new());
	slider.set_margin_top(5.0);
	slider.set_margin_left(4.0);
	slider.set_margin_right(4.0);
	slider.set_height(Length::Fixed(24.0));
	slider.set_width(Length::Stretch { min: 0.0, max: 600.0 });
	slider.set_horizontal_align(Alignment::Center);
	slider.set_steps(6, 1);
	slider
}

fn make_picture_widget(
	window: &Rc<Window>,
	slider: Rc<Slider>,
	bottom_container: Rc<HorizontalLayoutContainer>,
	config: Rc<RefCell<Configuration>>,
) -> Rc<PictureWidget> {
	let picture_widget = Rc::new(PictureWidget::new(
		&window.display_mut(),
		window,
		slider,
		bottom_container.clone(),
		config.clone(),
	));
	picture_widget.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
	picture_widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

	if let Some(file_path) = std::env::args().nth(1) {
		picture_widget.jump_to_path(file_path);
	}
	picture_widget
}

#[cfg(feature = "networking")]
#[derive(Deserialize)]
struct ReleaseInfoJson {
	tag_name: String,
}

fn get_config_and_cache_paths() -> (PathBuf, PathBuf) {
	let config_folder;
	let cache_folder;

	if let Some(ref project_dirs) = *PROJECT_DIRS {
		config_folder = project_dirs.config_dir().to_owned();
		cache_folder = project_dirs.cache_dir().to_owned();
	} else {
		let exe_path = std::env::current_exe().unwrap();
		let exe_folder = exe_path.parent().unwrap();
		config_folder = exe_folder.to_owned();
		cache_folder = exe_folder.to_owned();
	}
	if !config_folder.exists() {
		std::fs::create_dir_all(&config_folder).unwrap();
	}
	if !cache_folder.exists() {
		std::fs::create_dir_all(&cache_folder).unwrap();
	}

	(config_folder.join("cfg.toml"), cache_folder.join("cache.toml"))
}

#[cfg(not(feature = "networking"))]
/// Always returns false without the `networking` feature.
fn check_for_updates() -> bool {
	false
}

#[cfg(feature = "networking")]
/// Returns true if updates are available.
fn check_for_updates() -> bool {
	use std::str::FromStr;

	let client;
	match reqwest::blocking::Client::builder().user_agent("emulsion").build() {
		Ok(c) => client = c,
		Err(e) => {
			println!("Could not build client for version request: {}", e);
			return false;
		}
	}
	let response =
		client.get("https://api.github.com/repos/ArturKovacs/emulsion/releases/latest").send();
	match response {
		Ok(response) => match response.json::<ReleaseInfoJson>() {
			Ok(info) => {
				println!("Found latest version tag {}", info.tag_name);

				let current = Version::cargo_pkg_version();
				println!("Current version is '{}'", current);

				match Version::from_str(&info.tag_name) {
					Ok(latest) => {
						println!("Parsed latest version is '{}'", latest);

						if latest > current {
							return true;
						}
					}
					Err(error) => {
						println!("Error parsing version: {}", error.to_string());
					}
				}
			}
			Err(e) => println!("Failed to create json from response: {}", e),
		},
		Err(e) => println!("Failed to get latest version info: {}", e),
	}
	false
}
