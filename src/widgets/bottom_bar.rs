use super::picture_widget::ScalingMode;
use crate::{
	configuration::{Magnification},
	ConfigWindowSection, Configuration, Theme,
};

use gelatin::{
	button::Button,
	line_layout_container::HorizontalLayoutContainer,
	misc::{Alignment, Length},
	picture::Picture,
	slider::Slider,
};
use std::f32;
use std::rc::Rc;

static MOON: &[u8] = include_bytes!("../../resource/moon.png");
static LIGHT: &[u8] = include_bytes!("../../resource/light.png");
static QUESTION_BUTTON: &[u8] = include_bytes!("../../resource/question_button.png");
static QUESTION_BUTTON_LIGHT: &[u8] = include_bytes!("../../resource/question_button_light.png");
static QUESTION_NOTI: &[u8] = include_bytes!("../../resource/question-noti.png");
static QUESTION_LIGHT_NOTI: &[u8] = include_bytes!("../../resource/question-light-noti.png");
static ONE: &[u8] = include_bytes!("../../resource/1.png");
static ONE_LIGHT: &[u8] = include_bytes!("../../resource/1-light.png");
static FIT_STRETCH: &[u8] = include_bytes!("../../resource/fit-stretch.png");
static FIT_STRETCH_LIGHT: &[u8] = include_bytes!("../../resource/fit-stretch-light.png");
static FIT_BEST: &[u8] = include_bytes!("../../resource/fit-min.png");
static FIT_BEST_LIGHT: &[u8] = include_bytes!("../../resource/fit-min-light.png");
static MAGNIFY_PIXEL: &[u8] = include_bytes!("../../resource/magnify-pixel.png");
static MAGNIFY_PIXEL_LIGHT: &[u8] = include_bytes!("../../resource/magnify-pixel-light.png");
static MAGNIFY_SHARP: &[u8] = include_bytes!("../../resource/magnify-sharp.png");
static MAGNIFY_SHARP_LIGHT: &[u8] = include_bytes!("../../resource/magnify-sharp-light.png");

const NO_BG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const ACTIVE_BG_COLOR: [f32; 4] = [0.3, 0.3, 0.3, 0.5];

const SMALL_BUTTON_GAP: f32 = 4.0;
const BIG_BUTTON_GAP: f32 = 32.0;
const BUTTON_SIZE: f32 = 24.0;

pub struct BottomBar {
	pub widget: Rc<HorizontalLayoutContainer>,
	pub orig_scale_button: Rc<Button>,
	pub fit_stretch_button: Rc<Button>,
	pub fit_best_button: Rc<Button>,
	pub slider: Rc<Slider>,
	pub theme_button: Rc<Button>,
	pub help_button: Rc<Button>,
	pub magnification_button: Rc<Button>,

	/// This is false if the configuration requires this to be invisible
	// and true otherwise.
	pub should_show: bool,

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
	magnify_pixel: Rc<Picture>,
	magnify_pixel_light: Rc<Picture>,
	magnify_sharp: Rc<Picture>,
	magnify_sharp_light: Rc<Picture>,
}

impl BottomBar {
	pub fn new(config: &Configuration) -> Self {
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
		let magnify_pixel = Rc::new(Picture::from_encoded_bytes(MAGNIFY_PIXEL));
		let magnify_pixel_light = Rc::new(Picture::from_encoded_bytes(MAGNIFY_PIXEL_LIGHT));
		let magnify_sharp = Rc::new(Picture::from_encoded_bytes(MAGNIFY_SHARP));
		let magnify_sharp_light = Rc::new(Picture::from_encoded_bytes(MAGNIFY_SHARP_LIGHT));

		let widget = Rc::new(HorizontalLayoutContainer::new());
		widget.set_margin_left(0.0);
		widget.set_margin_right(0.0);
		widget.set_height(Length::Fixed(32.0));
		widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

		let orig_scale_button = make_icon_button(Alignment::Start);
		let fit_best_button = make_icon_button(Alignment::Start);
		let fit_stretch_button = make_icon_button(Alignment::Start);
		let slider = make_slider();
		let theme_button = make_icon_button(Alignment::End);
		let help_button = make_icon_button(Alignment::End);
		let magnification_button = make_icon_button(Alignment::End);

		orig_scale_button.set_margin_left(SMALL_BUTTON_GAP);
		fit_stretch_button.set_margin_right(SMALL_BUTTON_GAP);
		theme_button.set_margin_left(SMALL_BUTTON_GAP);
		help_button.set_margin_left(SMALL_BUTTON_GAP);
		help_button.set_margin_right(SMALL_BUTTON_GAP);

		widget.add_child(orig_scale_button.clone());
		widget.add_child(fit_best_button.clone());
		widget.add_child(fit_stretch_button.clone());
		widget.add_child(slider.clone());
		widget.add_child(magnification_button.clone());
		widget.add_child(theme_button.clone());
		widget.add_child(help_button.clone());

		let should_show;
		if let Some(ConfigWindowSection { show_bottom_bar: Some(false), .. }) = config.window {
			widget.set_visible(false);
			should_show = false;
		} else {
			should_show = true;
		}

		Self {
			widget,
			orig_scale_button,
			fit_stretch_button,
			fit_best_button,
			slider,
			theme_button,
			help_button,
			magnification_button,
			should_show,

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
			magnify_pixel,
			magnify_pixel_light,
			magnify_sharp,
			magnify_sharp_light,
		}
	}

	pub fn set_theme(&self, theme: Theme, update_available: bool, magnification: Magnification) {
		match theme {
			Theme::Light => {
				self.orig_scale_button.set_icon(Some(self.one.clone()));
				self.fit_best_button.set_icon(Some(self.fit_best.clone()));
				self.fit_stretch_button.set_icon(Some(self.fit_stretch.clone()));
				self.theme_button.set_icon(Some(self.moon_img.clone()));
				self.widget.set_bg_color([1.0, 1.0, 1.0, 1.0]);
				self.slider.set_shadow_color([0.0, 0.0, 0.0]);

				let magnification_icon = match magnification {
					Magnification::Pixel => self.magnify_pixel.clone(),
					Magnification::Sharp => self.magnify_sharp.clone(),
				};
				self.magnification_button.set_icon(Some(magnification_icon));
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

				let magnification_icon = match magnification {
					Magnification::Pixel => self.magnify_pixel_light.clone(),
					Magnification::Sharp => self.magnify_sharp_light.clone(),
				};
				self.magnification_button.set_icon(Some(magnification_icon));
				if update_available {
					self.help_button.set_icon(Some(self.question_light_noti.clone()));
				} else {
					self.help_button.set_icon(Some(self.question_light.clone()));
				}
			}
		}
	}

	/// Sets this visible iff both the `visible` parameter is `true` and
	/// the `should_show` property of this object is `true`
	pub fn set_visible_if_should_show(&self, visible: bool) {
		self.widget.set_visible(visible && self.should_show);
	}

	pub fn set_help_visible(&self, visible: bool) {
		self.help_button.set_bg_color(if visible { ACTIVE_BG_COLOR } else { NO_BG_COLOR })
	}

	pub fn update_scaling_buttons(&self, scaling: ScalingMode, img_texel_size: f32) {
		match scaling {
			#[allow(clippy::float_cmp)]
			ScalingMode::Fixed => {
				if img_texel_size == 1.0 {
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

fn make_icon_button(alignment: Alignment) -> Rc<Button> {
	let button = Rc::new(Button::new());
	button.set_margin_top(SMALL_BUTTON_GAP);
	button.set_height(Length::Fixed(BUTTON_SIZE));
	button.set_width(Length::Fixed(BUTTON_SIZE));
	button.set_horizontal_align(alignment);
	button
}

fn make_slider() -> Rc<Slider> {
	let slider = Rc::new(Slider::new());
	slider.set_margin_top(SMALL_BUTTON_GAP);
	slider.set_margin_left(BIG_BUTTON_GAP);
	slider.set_margin_right(BIG_BUTTON_GAP);
	slider.set_height(Length::Fixed(BUTTON_SIZE));
	slider.set_width(Length::Stretch { min: 0.0, max: std::f32::INFINITY });
	slider.set_horizontal_align(Alignment::Center);
	slider.set_steps(6, 1);
	slider
}
