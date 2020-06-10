use crate::Theme;

use gelatin::{
	button::Button,
	line_layout_container::HorizontalLayoutContainer,
	misc::{Alignment, Length},
	picture::Picture,
	slider::Slider,
};
use std::f32;
use std::rc::Rc;

static MOON: &[u8] = include_bytes!("../resource/moon.png");
static LIGHT: &[u8] = include_bytes!("../resource/light.png");
static QUESTION_BUTTON: &[u8] = include_bytes!("../resource/question_button.png");
static QUESTION_BUTTON_LIGHT: &[u8] = include_bytes!("../resource/question_button_light.png");
static QUESTION_NOTI: &[u8] = include_bytes!("../resource/question-noti.png");
static QUESTION_LIGHT_NOTI: &[u8] = include_bytes!("../resource/question-light-noti.png");
static ONE: &[u8] = include_bytes!("../resource/1.png");
static ONE_LIGHT: &[u8] = include_bytes!("../resource/1-light.png");
static FIT_STRETCH: &[u8] = include_bytes!("../resource/fit-stretch.png");
static FIT_STRETCH_LIGHT: &[u8] = include_bytes!("../resource/fit-stretch-light.png");
static FIT_BEST: &[u8] = include_bytes!("../resource/fit-min.png");
static FIT_BEST_LIGHT: &[u8] = include_bytes!("../resource/fit-min-light.png");

pub struct BottomBar {
	pub widget: Rc<HorizontalLayoutContainer>,
	pub orig_scale_button: Rc<Button>,
	pub fit_stretch_button: Rc<Button>,
	pub fit_best_button: Rc<Button>,
	pub slider: Rc<Slider>,
	pub theme_button: Rc<Button>,
	pub help_button: Rc<Button>,

	question: Rc<Picture>,
	question_light: Rc<Picture>,
	question_noti: Rc<Picture>,
	question_light_noti: Rc<Picture>,
	moon_img: Rc<Picture>,
	light_img: Rc<Picture>,
	one: Rc<Picture>,
	one_light: Rc<Picture>,
	fit_stretch: Rc<Picture>,
	fit_stretch_light: Rc<Picture>,
	fit_best: Rc<Picture>,
	fit_best_light: Rc<Picture>,
}

impl BottomBar {
	pub fn new() -> Self {
		let question = Rc::new(Picture::from_encoded_bytes(QUESTION_BUTTON));
		let question_light = Rc::new(Picture::from_encoded_bytes(QUESTION_BUTTON_LIGHT));
		let question_noti = Rc::new(Picture::from_encoded_bytes(QUESTION_NOTI));
		let question_light_noti = Rc::new(Picture::from_encoded_bytes(QUESTION_LIGHT_NOTI));
		let moon_img = Rc::new(Picture::from_encoded_bytes(MOON));
		let light_img = Rc::new(Picture::from_encoded_bytes(LIGHT));
		let one = Rc::new(Picture::from_encoded_bytes(ONE));
		let one_light = Rc::new(Picture::from_encoded_bytes(ONE_LIGHT));
		let fit_stretch = Rc::new(Picture::from_encoded_bytes(FIT_STRETCH));
		let fit_stretch_light = Rc::new(Picture::from_encoded_bytes(FIT_STRETCH_LIGHT));
		let fit_best = Rc::new(Picture::from_encoded_bytes(FIT_BEST));
		let fit_best_light = Rc::new(Picture::from_encoded_bytes(FIT_BEST_LIGHT));

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

		let orig_scale_button = make_orig_scale_button();
		let fit_best_button = make_fit_best_button();
		let fit_stretch_button = make_fit_stretch_button();

		widget.add_child(orig_scale_button.clone());
		widget.add_child(fit_best_button.clone());
		widget.add_child(fit_stretch_button.clone());

		widget.add_child(slider.clone());
		widget.add_child(theme_button.clone());
		widget.add_child(help_button.clone());

		Self {
			widget,
			orig_scale_button,
			fit_stretch_button,
			fit_best_button,
			slider,
			theme_button,
			help_button,

			question,
			question_light,
			question_noti,
			question_light_noti,
			moon_img,
			light_img,
			one,
			one_light,
			fit_stretch,
			fit_stretch_light,
			fit_best,
			fit_best_light,
		}
	}

	pub fn set_theme(&self, theme: Theme, update_available: bool) {
		match theme {
			Theme::Light => {
				self.orig_scale_button.set_icon(Some(self.one.clone()));
				self.fit_best_button.set_icon(Some(self.fit_best.clone()));
				self.fit_stretch_button.set_icon(Some(self.fit_stretch.clone()));
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
				self.orig_scale_button.set_icon(Some(self.one_light.clone()));
				self.fit_best_button.set_icon(Some(self.fit_best_light.clone()));
				self.fit_stretch_button.set_icon(Some(self.fit_stretch_light.clone()));
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

fn make_orig_scale_button() -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(4.0);
	button.set_margin_left(4.0);
	button.set_margin_right(0.0);
	button.set_height(Length::Fixed(24.0));
	button.set_width(Length::Fixed(24.0));
	button.set_horizontal_align(Alignment::Start);
	button
}

fn make_fit_best_button() -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(4.0);
	button.set_margin_left(0.0);
	button.set_margin_right(0.0);
	button.set_height(Length::Fixed(24.0));
	button.set_width(Length::Fixed(24.0));
	button.set_horizontal_align(Alignment::Start);
	button
}

fn make_fit_stretch_button() -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(4.0);
	button.set_margin_left(0.0);
	button.set_margin_right(32.0);
	button.set_height(Length::Fixed(24.0));
	button.set_width(Length::Fixed(24.0));
	button.set_bg_color([0.4, 0.4, 0.4, 0.5]);
	button.set_horizontal_align(Alignment::Start);
	button
}

fn make_theme_button() -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(4.0);
	button.set_margin_left(32.0);
	button.set_margin_right(4.0);
	button.set_height(Length::Fixed(24.0));
	button.set_width(Length::Fixed(24.0));
	button.set_horizontal_align(Alignment::End);
	button
}

fn make_help_button() -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(4.0);
	button.set_margin_left(4.0);
	button.set_margin_right(4.0);
	button.set_height(Length::Fixed(24.0));
	button.set_width(Length::Fixed(24.0));
	button.set_horizontal_align(Alignment::End);
	button
}

fn make_slider() -> Rc<Slider> {
	let slider = Rc::new(Slider::new());
	slider.set_margin_top(4.0);
	slider.set_margin_left(4.0);
	slider.set_margin_right(4.0);
	slider.set_height(Length::Fixed(24.0));
	slider.set_width(Length::Stretch { min: 0.0, max: std::f32::INFINITY });
	slider.set_horizontal_align(Alignment::Center);
	slider.set_steps(6, 1);
	slider
}
