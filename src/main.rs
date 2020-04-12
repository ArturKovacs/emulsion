#![windows_subsystem = "windows"]

#[macro_use]
extern crate error_chain;
extern crate backtrace;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate rmp_serde;
extern crate rand;
extern crate alphanumeric_sort;
extern crate trash;

use std::env;
use std::rc::Rc;

use gelatin::glium::{glutin, Surface};

mod handle_panic;
mod image_cache;
mod shaders;

mod picture_widget;
use crate::picture_widget::*;

mod playback_manager;
use crate::playback_manager::{LoadRequest, PlaybackManager};

mod configuration;
use crate::configuration::Configuration;

mod util;

use std::cell::Cell;
use std::f32;
use gelatin::{
    application::*, button::*, line_layout_container::*, misc::*, picture::*, slider::*, window::Window
};

// ========================================================
// Glorious main function
// ========================================================
fn main() {
    std::panic::set_hook(Box::new(handle_panic::handle_panic));

    let mut application = Application::new();
    let window = gelatin::window::Window::new(&mut application);
    let vertical_container = Rc::new(VerticalLayoutContainer::new());
    vertical_container.set_margin_all(0.0);
    vertical_container.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
    vertical_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    let bottom_container = Rc::new(HorizontalLayoutContainer::new());
    bottom_container.set_margin_top(4.0);
    bottom_container.set_margin_bottom(4.0);
    bottom_container.set_height(Length::Fixed(32.0));
    bottom_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    let moon = Rc::new(Picture::new("resource/moon.png"));
    let light = Rc::new(Picture::new("resource/light.png"));
    let theme_button = Rc::new(Button::new());
    theme_button.set_margin_top(5.0);
    theme_button.set_height(Length::Fixed(24.0));
    theme_button.set_width(Length::Fixed(24.0));
    theme_button.set_horizontal_align(Alignment::Center);
    theme_button.set_icon(Some(moon.clone()));

    let question = Rc::new(Picture::new("resource/question_button.png"));
    let question_light = Rc::new(Picture::new("resource/question_button_light.png"));
    let help_button = Rc::new(Button::new());
    help_button.set_margin_top(5.0);
    help_button.set_height(Length::Fixed(24.0));
    help_button.set_width(Length::Fixed(24.0));
    help_button.set_horizontal_align(Alignment::Center);
    help_button.set_icon(Some(question.clone()));

    let slider = Rc::new(Slider::new());
    slider.set_margin_top(5.0);
    slider.set_height(Length::Fixed(24.0));
    slider.set_width(Length::Stretch { min: 0.0, max: 600.0 });
    slider.set_horizontal_align(Alignment::Center);
    slider.set_steps(6, 1);

    let image_widget = Rc::new(PictureWidget::new(&window.display_mut(), slider.clone()));
    image_widget.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
    image_widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    bottom_container.add_child(theme_button.clone());
    bottom_container.add_child(slider.clone());
    bottom_container.add_child(help_button.clone());

    vertical_container.add_child(image_widget.clone());
    vertical_container.add_child(bottom_container.clone());

    bottom_container.set_margin_left(0.0);
    bottom_container.set_margin_right(0.0);
    theme_button.set_margin_left(4.0);
    theme_button.set_margin_right(4.0);
    help_button.set_margin_left(4.0);
    help_button.set_margin_right(4.0);
    slider.set_margin_left(4.0);
    slider.set_margin_right(4.0);

    let theme_button_clone = theme_button.clone();
    let help_button_clone = help_button.clone();
    let slider_clone = slider.clone();
    let window_clone = window.clone();
    let light_theme = Cell::new(true);
    theme_button.set_on_click(move || {
        light_theme.set(!light_theme.get());
        set_theme(
            light_theme.get(),
            &slider_clone,
            &theme_button_clone,
            &help_button_clone,
            &window_clone,
            &moon,
            &light,
            &question,
            &question_light
        );
    });
    let slider_clone2 = slider.clone();
    let image_widget_clone = image_widget.clone();
    slider.set_on_value_change(move || {
        image_widget_clone.jump_to_index(slider_clone2.value());
    });
    window.set_root(vertical_container);
    application.start_event_loop();
}
// ========================================================

fn set_theme(
    light_theme: bool,
    slider: &Rc<Slider>,
    theme_button: &Rc<Button>,
    help_button: &Rc<Button>,
    window: &Window,

    moon_texture: &Rc<Picture>,
    light_texture: &Rc<Picture>,
    question_texture: &Rc<Picture>,
    question_texture_light: &Rc<Picture>,
) {
    
    if light_theme {
        slider.set_shadow_color([0.0, 0.0, 0.0]);
        window.set_bg_color([0.85, 0.85, 0.85, 1.0]);
        theme_button.set_icon(Some(moon_texture.clone()));
        help_button.set_icon(Some(question_texture.clone()));
    } else {
        slider.set_shadow_color([0.0, 0.0, 0.0]);
        window.set_bg_color([0.05, 0.05, 0.05, 1.0]);
        theme_button.set_icon(Some(light_texture.clone()));
        help_button.set_icon(Some(question_texture_light.clone()));
    }
}
