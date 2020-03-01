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
    application::*, button::*, line_layout_container::*, misc::*, picture::*, slider::*
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
    let image_widget = Rc::new(PictureWidget::new(&window.display_mut()));
    image_widget.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
    image_widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    let bottom_container = Rc::new(HorizontalLayoutContainer::new());
    bottom_container.set_margin_top(4.0);
    bottom_container.set_margin_bottom(4.0);
    bottom_container.set_height(Length::Fixed(32.0));
    bottom_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    let moon = Rc::new(Picture::new("resource/moon.png"));
    let theme_button = Rc::new(Button::new());
    theme_button.set_margin_top(5.0);
    theme_button.set_height(Length::Fixed(24.0));
    theme_button.set_width(Length::Fixed(24.0));
    theme_button.set_horizontal_align(Alignment::Center);
    theme_button.set_icon(Some(moon));

    let question = Rc::new(Picture::new("resource/question_button.png"));
    let help_button = Rc::new(Button::new());
    help_button.set_margin_top(5.0);
    help_button.set_height(Length::Fixed(24.0));
    help_button.set_width(Length::Fixed(24.0));
    help_button.set_horizontal_align(Alignment::Center);
    help_button.set_icon(Some(question));

    let slider = Rc::new(Slider::new());
    slider.set_margin_top(5.0);
    slider.set_height(Length::Fixed(24.0));
    slider.set_width(Length::Stretch { min: 0.0, max: 600.0 });
    slider.set_horizontal_align(Alignment::Center);
    slider.set_steps(6);

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

    let button_clone = theme_button.clone();
    let pos = Cell::new(5.0);
    theme_button.set_on_click(move || {
        let new_pos = pos.get() + 5.0;
        pos.set(new_pos);

        button_clone.set_margin_left(new_pos);
        button_clone.set_margin_top(new_pos);
    });
    let button_clone2 = theme_button.clone();
    let slider_clone = slider.clone();
    slider.set_on_value_change(move || {
        let margin = (slider_clone.value() + 1) as f32 * 5.0;
        button_clone2.set_margin_right(margin);
    });
    window.set_root(vertical_container);
    application.start_event_loop();
}
// ========================================================
