//#![windows_subsystem = "windows"]

#[macro_use]
extern crate error_chain;

use gelatin::image;

use std::cell::RefCell;
use std::rc::Rc;

use gelatin::glium::glutin::{
	dpi::{PhysicalPosition, PhysicalSize},
	event::WindowEvent,
	window::Icon,
};

mod handle_panic;
mod image_cache;
mod shaders;

mod picture_widget;
use crate::picture_widget::*;

mod help_screen;
use crate::help_screen::*;

mod playback_manager;

mod configuration;
use crate::configuration::Configuration;

use gelatin::{
	application::*,
    button::*,
    label::*,
	line_layout_container::*,
	misc::*,
	picture::*,
	slider::*,
	window::{Window, WindowDescriptorBuilder},
};
use std::cell::Cell;
use std::f32;

// ========================================================
// Glorious main function
// ========================================================
fn main() {
	std::panic::set_hook(Box::new(handle_panic::handle_panic));

	let img = image::open("resource/emulsion48.png").unwrap();
	let rgba = img.into_rgba();
	let (w, h) = rgba.dimensions();
	let icon = Icon::from_rgba(rgba.into_raw(), w, h).unwrap();

	let cfg_path = "cfg.toml";
	let first_lanuch;
	let config: Rc<RefCell<Configuration>>;
	if let Ok(cfg) = Configuration::load(cfg_path) {
		first_lanuch = false;
		config = Rc::new(RefCell::new(cfg));
	} else {
		first_lanuch = true;
		config = Rc::new(RefCell::new(Configuration::default()));
	}
	let mut application = Application::new();
	{
		let config_clone = config.clone();
		application.set_at_exit(Some(move || {
			config_clone.borrow().save(cfg_path).unwrap();
		}));
	}
	let window: Rc<Window>;
	{
		let config = config.borrow();
		let window_desc = WindowDescriptorBuilder::default()
			.icon(Some(icon))
			.size(PhysicalSize::new(config.win_w, config.win_h))
			.position(Some(PhysicalPosition::new(config.win_x, config.win_y)))
			.build()
			.unwrap();
		window = Window::new(&mut application, window_desc);
	}
	{
		let config_clone = config.clone();
		window.add_global_event_handler(move |event| match event {
			WindowEvent::Resized(new_size) => {
				let mut config = config_clone.borrow_mut();
				config.win_w = new_size.width;
				config.win_h = new_size.height;
			}
			WindowEvent::Moved(new_pos) => {
				let mut config = config_clone.borrow_mut();
				config.win_x = new_pos.x;
				config.win_y = new_pos.y;
			}
			_ => (),
		});
	}
	let vertical_container = Rc::new(VerticalLayoutContainer::new());
	vertical_container.set_margin_all(0.0);
	vertical_container.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
	vertical_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

	let picture_area_container = Rc::new(VerticalLayoutContainer::new());
	picture_area_container.set_margin_all(0.0);
	picture_area_container.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
	picture_area_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    let update_notification = Rc::new(HorizontalLayoutContainer::new());
    let update_label = Rc::new(Label::new());
    let update_label_image = Rc::new(Picture::new("resource/new-version-available.png"));
    let update_label_image_light = Rc::new(Picture::new("resource/new-version-available-light.png"));
    {
        update_notification.set_vertical_align(Alignment::End);
        update_notification.set_horizontal_align(Alignment::Start);
        update_notification.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
        update_notification.set_height(Length::Fixed(32.0));
        update_label.set_icon(Some(update_label_image.clone()));
        update_label.set_margin_top(4.0);
        update_label.set_margin_bottom(4.0);
        update_label.set_fixed_size(LogicalVector::new(200.0, 24.0));
        update_label.set_horizontal_align(Alignment::Center);
        let update_button = Rc::new(Button::new());
        let button_image = Rc::new(Picture::new("resource/visit-site.png"));
        update_button.set_icon(Some(button_image));
        update_button.set_margin_top(4.0);
        update_button.set_margin_bottom(4.0);
        update_button.set_fixed_size(LogicalVector::new(100.0, 24.0));
        update_button.set_horizontal_align(Alignment::Center);
        update_notification.add_child(update_label.clone());
        update_notification.add_child(update_button);
    }
    
	let help_screen = Rc::new(HelpScreen::new());

	let bottom_container = Rc::new(HorizontalLayoutContainer::new());
	//bottom_container.set_margin_top(4.0);
	//bottom_container.set_margin_bottom(4.0);
	bottom_container.set_margin_left(0.0);
	bottom_container.set_margin_right(0.0);
	bottom_container.set_height(Length::Fixed(32.0));
	bottom_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

	let moon_img = Rc::new(Picture::new("resource/moon.png"));
	let light_img = Rc::new(Picture::new("resource/light.png"));
	let theme_button = Rc::new(Button::new());
	theme_button.set_margin_top(5.0);
	theme_button.set_margin_left(28.0);
	theme_button.set_margin_right(4.0);
	theme_button.set_height(Length::Fixed(24.0));
	theme_button.set_width(Length::Fixed(24.0));
	theme_button.set_horizontal_align(Alignment::Center);
	theme_button.set_icon(Some(moon_img.clone()));

	let question = Rc::new(Picture::new("resource/question_button.png"));
    let question_light = Rc::new(Picture::new("resource/question_button_light.png"));
    let question_noti = Rc::new(Picture::new("resource/question-noti.png"));
    let question_light_noti = Rc::new(Picture::new("resource/question-light-noti.png"));
	let help_button = Rc::new(Button::new());
	help_button.set_margin_top(5.0);
	help_button.set_margin_left(4.0);
	help_button.set_margin_right(28.0);
	help_button.set_height(Length::Fixed(24.0));
	help_button.set_width(Length::Fixed(24.0));
	help_button.set_horizontal_align(Alignment::Center);
	help_button.set_icon(Some(question.clone()));

	let slider = Rc::new(Slider::new());
	slider.set_margin_top(5.0);
	slider.set_margin_left(4.0);
	slider.set_margin_right(4.0);
	slider.set_height(Length::Fixed(24.0));
	slider.set_width(Length::Stretch { min: 0.0, max: 600.0 });
	slider.set_horizontal_align(Alignment::Center);
	slider.set_steps(6, 1);

	let picture_widget = Rc::new(PictureWidget::new(
		&window.display_mut(),
		&window,
		slider.clone(),
		bottom_container.clone(),
	));
	picture_widget.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
	picture_widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
	if let Some(file_path) = std::env::args().nth(1) {
		picture_widget.jump_to_path(file_path);
	}

	bottom_container.add_child(theme_button.clone());
	bottom_container.add_child(slider.clone());
	bottom_container.add_child(help_button.clone());

	picture_area_container.add_child(picture_widget.clone());
    picture_area_container.add_child(help_screen.clone());
    picture_area_container.add_child(update_notification.clone());

	vertical_container.add_child(picture_area_container);
	vertical_container.add_child(bottom_container.clone());

	let theme_button_clone = theme_button.clone();
    let help_button_clone = help_button.clone();
    let update_label_clone = update_label.clone();
	let picture_widget_clone = picture_widget.clone();
    let bottom_container_clone = bottom_container;
    let update_notification_clone = update_notification.clone();
	let slider_clone = slider.clone();
	let window_clone = window.clone();
    let light_theme = Cell::new(!config.borrow().dark);
    let update_available = Rc::new(Cell::new(true));
    let set_theme = move |light: bool, update: bool| {
        if light {
            picture_widget_clone.set_bright_shade(0.96);
            bottom_container_clone.set_bg_color([1.0, 1.0, 1.0, 1.0]);
            slider_clone.set_shadow_color([0.0, 0.0, 0.0]);
            window_clone.set_bg_color([0.85, 0.85, 0.85, 1.0]);
            theme_button_clone.set_icon(Some(moon_img.clone()));
            update_notification_clone.set_bg_color([0.06, 0.06, 0.06, 1.0]);
            update_label_clone.set_icon(Some(update_label_image_light.clone()));
            if update {
                help_button_clone.set_icon(Some(question_noti.clone()));
            } else {
                help_button_clone.set_icon(Some(question.clone()));
            }
        } else {
            picture_widget_clone.set_bright_shade(0.3);
            bottom_container_clone.set_bg_color([0.1, 0.1, 0.1, 1.0]);
            slider_clone.set_shadow_color([0.0, 0.0, 0.0]);
            window_clone.set_bg_color([0.05, 0.05, 0.05, 1.0]);
            theme_button_clone.set_icon(Some(light_img.clone()));
            update_notification_clone.set_bg_color([0.85, 0.85, 0.85, 1.0]);
            update_label_clone.set_icon(Some(update_label_image.clone()));
            if update {
                help_button_clone.set_icon(Some(question_light_noti.clone()));
            } else {
                help_button_clone.set_icon(Some(question_light.clone()));
            }
        }
    };
    set_theme(light_theme.get(), update_available.get());
    let update_available_clone = update_available.clone();
	theme_button.set_on_click(move || {
		light_theme.set(!light_theme.get());
		config.borrow_mut().dark = !light_theme.get();
		set_theme(light_theme.get(), update_available_clone.get());
	});
	let slider_clone2 = slider.clone();
	let image_widget_clone = picture_widget;
	slider.set_on_value_change(move || {
		image_widget_clone.jump_to_index(slider_clone2.value());
	});
    let help_visible = Cell::new(first_lanuch);
    help_screen.set_visible(help_visible.get());
    update_notification.set_visible(help_visible.get() && update_available.get());
	help_button.set_on_click(move || {
        help_visible.set(!help_visible.get());
		help_screen.set_visible(help_visible.get());
        update_notification.set_visible(help_visible.get() && update_available.get());
	});
	window.set_root(vertical_container);
	application.start_event_loop();
}
// ========================================================

fn check_for_updates(update_notification: Rc<HorizontalLayoutContainer>, ) {
    
}
