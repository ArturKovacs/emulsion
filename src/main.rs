//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
extern crate error_chain;

use std::cell::{Cell, RefCell};
use std::f32;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use directories_next::ProjectDirs;
use lazy_static::lazy_static;

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
	window::{Window, WindowDescriptorBuilder},
	NextUpdate, Widget,
};

use crate::configuration::Theme;
use crate::configuration::{Cache, Configuration};
use crate::version::Version;
use crate::widgets::{
	bottom_bar::BottomBar, copy_notification::CopyNotifications, help_screen::*, picture_widget::*,
};

mod clipboard_handler;
mod cmd_line;
mod configuration;
mod handle_panic;
mod image_cache;
mod input_handling;
mod parallel_action;
mod playback_manager;
mod shaders;
mod utils;
mod version;
mod widgets;

lazy_static! {
	// The program name will be 'emulsion'
	// (i.e. starting with a lower-case letter) on Linux
	pub static ref PROJECT_DIRS: Option<ProjectDirs> = ProjectDirs::from("", "", "Emulsion");
}

static NEW_VERSION: &[u8] = include_bytes!("../resource/new-version-available.png");
static NEW_VERSION_LIGHT: &[u8] = include_bytes!("../resource/new-version-available-light.png");
static VISIT_SITE: &[u8] = include_bytes!("../resource/visit-site.png");
static USAGE: &[u8] = include_bytes!("../resource/usage.png");
static LEFT_TO_PAN: &[u8] = include_bytes!("../resource/use-left-to-pan.png");

// ========================================================
// Not-so glorious main function
// ========================================================
fn main() {
	std::panic::set_hook(Box::new(handle_panic::handle_panic));

	// Load configuration and cache files
	let (config_path, cache_path) = get_config_and_cache_paths();

	let args = cmd_line::parse_args(&config_path, &cache_path);

	let cache = Cache::load(&cache_path);
	let config = Configuration::load(&config_path);

	let first_launch = cache.is_err();
	let cache = Arc::new(Mutex::new(cache.unwrap_or_default()));
	let config = Rc::new(RefCell::new(config.unwrap_or_default()));

	if args.displayed_folders.is_some() {
		config.borrow_mut().title.get_or_insert_with(Default::default).displayed_folders =
			args.displayed_folders;
	}

	let mut application = Application::new();
	let window: Rc<Window> = {
		let window = &mut cache.lock().unwrap().window;

		let window_defaults = configuration::WindowSection::default();
		let right = window.win_x as i64 + window.win_w as i64;
		if right < 20 {
			window.win_w = window_defaults.win_w;
			window.win_x = window_defaults.win_x;
		}
		if window.win_y < 20 {
			window.win_y = window_defaults.win_y;
		}
		let window_desc = WindowDescriptorBuilder::default()
			.icon(Some(make_icon()))
			.size(PhysicalSize::new(window.win_w, window.win_h))
			.position(Some(PhysicalPosition::new(window.win_x, window.win_y)))
			.build()
			.unwrap();
		Window::new(&mut application, window_desc)
	};
	add_window_movement_listener(&window, cache.clone());

	let update_label_image = Rc::new(Picture::from_encoded_bytes(NEW_VERSION));
	let update_label_image_light = Rc::new(Picture::from_encoded_bytes(NEW_VERSION_LIGHT));
	let update_label = make_update_label();

	let update_notification = make_update_notification(update_label.clone());

	let usage_img = Picture::from_encoded_bytes(USAGE);
	let help_screen = Rc::new(HelpScreen::new(usage_img));
	let left_to_pan_img = Picture::from_encoded_bytes(LEFT_TO_PAN);
	let left_to_pan_hint = Rc::new(HelpScreen::new(left_to_pan_img));

	let copy_notifications_widget = Rc::new(Label::new());
	let copy_notifications = CopyNotifications::new(&copy_notifications_widget);

	let bottom_bar = Rc::new(BottomBar::new());
	let picture_widget = make_picture_widget(
		&window,
		bottom_bar.clone(),
		left_to_pan_hint.clone(),
		copy_notifications,
		config.clone(),
		cache.clone(),
	);

	if let Some(file_path) = args.file_path {
		picture_widget.jump_to_path(file_path);
	}

	let picture_area_container = make_picture_area_container();
	picture_area_container.add_child(picture_widget.clone());
	picture_area_container.add_child(copy_notifications_widget);
	picture_area_container.add_child(left_to_pan_hint);
	picture_area_container.add_child(help_screen.clone());
	picture_area_container.add_child(update_notification.clone());

	let root_container = make_root_container();
	root_container.add_child(picture_area_container);
	root_container.add_child(bottom_bar.widget.clone());

	let update_available = Arc::new(AtomicBool::new(false));
	let update_check_done = Arc::new(AtomicBool::new(false));
	let theme = Rc::new(Cell::new(cache.lock().unwrap().theme()));

	let set_theme = {
		let update_label = update_label;
		let picture_widget = picture_widget.clone();
		let update_notification = update_notification.clone();
		let window = window.clone();
		let theme = theme.clone();
		let update_available = update_available.clone();
		let bottom_bar = bottom_bar.clone();

		Rc::new(move || {
			match theme.get() {
				Theme::Light => {
					picture_widget.set_bright_shade(0.96);
					window.set_bg_color([0.85, 0.85, 0.85, 1.0]);
					update_notification.set_bg_color([0.06, 0.06, 0.06, 1.0]);
					update_label.set_icon(Some(update_label_image_light.clone()));
				}
				Theme::Dark => {
					picture_widget.set_bright_shade(0.11);
					window.set_bg_color([0.03, 0.03, 0.03, 1.0]);
					update_notification.set_bg_color([0.85, 0.85, 0.85, 1.0]);
					update_label.set_icon(Some(update_label_image.clone()));
				}
			}
			bottom_bar.set_theme(theme.get(), update_available.load(Ordering::SeqCst));
		})
	};
	set_theme();
	{
		let cache = cache.clone();
		let set_theme = set_theme.clone();
		bottom_bar.theme_button.set_on_click(move || {
			let new_theme = theme.get().switch_theme();
			theme.set(new_theme);
			cache.lock().unwrap().set_theme(new_theme);
			set_theme();
		});
	}
	{
		let slider = bottom_bar.slider.clone();
		let picture_widget = picture_widget.clone();
		bottom_bar.slider.set_on_value_change(move || {
			picture_widget.jump_to_index(slider.value());
		});
	}
	{
		let picture_widget = picture_widget.clone();
		bottom_bar.orig_scale_button.set_on_click(move || {
			picture_widget.set_img_size_to_orig();
		});
	}
	{
		let picture_widget = picture_widget.clone();
		bottom_bar.fit_best_button.set_on_click(move || {
			picture_widget.set_img_size_to_fit(false);
		});
	}
	{
		let picture_widget = picture_widget.clone();
		bottom_bar.fit_stretch_button.set_on_click(move || {
			picture_widget.set_img_size_to_fit(true);
		});
	}
	let help_visible = Cell::new(first_launch);
	help_screen.set_visible(help_visible.get());
	update_notification.set_visible(help_visible.get() && update_available.load(Ordering::SeqCst));
	{
		let update_available = update_available.clone();
		let help_screen = help_screen.clone();
		let update_notification = update_notification.clone();
		let bottom_bar_clone = bottom_bar.clone();

		bottom_bar.help_button.set_on_click(move || {
			help_visible.set(!help_visible.get());
			help_screen.set_visible(help_visible.get());
			bottom_bar_clone.set_help_visible(help_visible.get());
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
				let has_update = update::check_for_updates();
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

fn add_window_movement_listener(window: &Window, cache: Arc<Mutex<Cache>>) {
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

fn make_picture_widget(
	window: &Rc<Window>,
	bottom_bar: Rc<BottomBar>,
	left_to_pan_hint: Rc<HelpScreen>,
	copy_notifications: CopyNotifications,
	config: Rc<RefCell<Configuration>>,
	cache: Arc<Mutex<Cache>>,
) -> Rc<PictureWidget> {
	let picture_widget = Rc::new(PictureWidget::new(
		&window.display_mut(),
		window,
		bottom_bar,
		left_to_pan_hint,
		copy_notifications,
		config,
		cache,
	));
	picture_widget.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
	picture_widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
	picture_widget
}

pub fn get_config_and_cache_paths() -> (PathBuf, PathBuf) {
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
mod update {
	/// Always returns false without the `networking` feature.
	pub fn check_for_updates() -> bool {
		false
	}
}

#[cfg(feature = "networking")]
mod update {
	use serde::Deserialize;

	#[derive(Deserialize)]
	struct ReleaseInfoJson {
		tag_name: String,
	}

	mod errors {
		error_chain! {
			foreign_links {
				Io(std::io::Error);
				ParseIntError(std::num::ParseIntError);
			}
		}
	}

	/// Tries to fetch latest release tag
	fn latest_release() -> errors::Result<ReleaseInfoJson> {
		let url = "https://api.github.com/repos/ArturKovacs/emulsion/releases/latest";
		let res = ureq::get(&url).set("User-Agent", "emulsion").call();
		if res.ok() {
			let release_info = res.into_json_deserialize()?;
			Ok(release_info)
		} else {
			Err(res.status_line().into())
		}
	}

	/// Tries to parse version tag and compare against current version
	fn compare_release(info: &ReleaseInfoJson) -> errors::Result<bool> {
		use crate::version::Version;
		use std::str::FromStr;

		let current = Version::cargo_pkg_version();
		let latest = Version::from_str(&info.tag_name)?;

		if latest > current {
			println!("Current version is {}, latest version is {}", current, latest);
			Ok(true)
		} else {
			Ok(false)
		}
	}

	/// Returns true if updates are available.
	pub fn check_for_updates() -> bool {
		match latest_release() {
			Ok(info) => match compare_release(&info) {
				Ok(is_newer) => is_newer,
				Err(err) => {
					eprintln!("Error parsing release tag: {}", err);
					false
				}
			},
			Err(err) => {
				eprintln!("Error checking latest release: {}", err);
				false
			}
		}
	}
}
