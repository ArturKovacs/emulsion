use crate::Theme;

use gelatin::{
	button::Button,
	line_layout_container::HorizontalLayoutContainer,
	misc::{Alignment, Length},
	picture::Picture,
	slider::Slider,
};
use std::rc::Rc;

static MOON: &[u8] = include_bytes!("../resource/moon.png");
static LIGHT: &[u8] = include_bytes!("../resource/light.png");
static QUESTION_BUTTON: &[u8] = include_bytes!("../resource/question_button.png");
static QUESTION_BUTTON_LIGHT: &[u8] = include_bytes!("../resource/question_button_light.png");
static QUESTION_NOTI: &[u8] = include_bytes!("../resource/question-noti.png");
static QUESTION_LIGHT_NOTI: &[u8] = include_bytes!("../resource/question-light-noti.png");

pub struct BottomBar {
	widget: Rc<HorizontalLayoutContainer>,
	theme_button: Rc<Button>,
	slider: Rc<Slider>,
	help_button: Rc<Button>,

	question: Rc<Picture>,
	question_light: Rc<Picture>,
	question_noti: Rc<Picture>,
	question_light_noti: Rc<Picture>,
	moon_img: Rc<Picture>,
	light_img: Rc<Picture>,
}

impl BottomBar {
	pub fn new() -> Self {
		let question = Rc::new(Picture::from_encoded_bytes(QUESTION_BUTTON));
		let question_light = Rc::new(Picture::from_encoded_bytes(QUESTION_BUTTON_LIGHT));
		let question_noti = Rc::new(Picture::from_encoded_bytes(QUESTION_NOTI));
		let question_light_noti = Rc::new(Picture::from_encoded_bytes(QUESTION_LIGHT_NOTI));
		let moon_img = Rc::new(Picture::from_encoded_bytes(MOON));
		let light_img = Rc::new(Picture::from_encoded_bytes(LIGHT));

		let theme_button = make_theme_button();
		let help_button = make_help_button();
		let slider = make_slider();

		let widget = Rc::new(HorizontalLayoutContainer::new());
		// widget.set_margin_top(4.0);
		// widget.set_margin_bottom(4.0);
		widget.set_margin_left(0.0);
		widget.set_margin_right(0.0);
		widget.set_height(Length::Fixed(32.0));
		widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

		widget.add_child(theme_button.clone());
		widget.add_child(slider.clone());
		widget.add_child(help_button.clone());

		Self {
			widget,
			theme_button,
			slider,
			help_button,

			question,
			question_light,
			question_noti,
			question_light_noti,
			moon_img,
			light_img,
		}
	}

	pub fn widget(&self) -> Rc<HorizontalLayoutContainer> {
		self.widget.clone()
	}

	pub fn slider(&self) -> Rc<Slider> {
		self.slider.clone()
	}

	pub fn set_on_slider_value_change<T: Fn() + 'static>(&self, callback: T) {
		self.slider.set_on_value_change(callback);
	}

	pub fn set_on_help_click<T: Fn() + 'static>(&self, callback: T) {
		self.help_button.set_on_click(callback);
	}

	pub fn set_on_theme_click<T: Fn() + 'static>(&self, callback: T) {
		self.theme_button.set_on_click(callback);
	}

	pub fn set_theme(&self, theme: Theme, update_available: bool) {
		match theme {
			Theme::Light => {
				self.theme_button.set_icon(Some(self.moon_img.clone()));
				self.widget.set_bg_color([1.0, 1.0, 1.0, 1.0]);
				self.slider.set_shadow_color([0.0, 0.0, 0.0]);

				if update_available {
					self.help_button.set_icon(Some(self.question_noti.clone()));
				} else {
					self.help_button.set_icon(Some(self.question.clone()));
				}
			}
			Theme::Dark => {
				self.theme_button.set_icon(Some(self.light_img.clone()));
				self.widget.set_bg_color([0.08, 0.08, 0.08, 1.0]);
				self.slider.set_shadow_color([0.0, 0.0, 0.0]);

				if update_available {
					self.help_button.set_icon(Some(self.question_light_noti.clone()));
				} else {
					self.help_button.set_icon(Some(self.question_light.clone()));
				}
			}
		}
	}
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
