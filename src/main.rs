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

mod help_screen;
use crate::help_screen::*;

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

    let picture_area_container = Rc::new(VerticalLayoutContainer::new());
    picture_area_container.set_margin_all(0.0);
    picture_area_container.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
    picture_area_container.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });

    let help_screen = Rc::new(HelpScreen::new());

    let bottom_container = Rc::new(HorizontalLayoutContainer::new());
    //bottom_container.set_margin_top(4.0);
    //bottom_container.set_margin_bottom(4.0);
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
    
    let picture_widget = Rc::new(PictureWidget::new(&window.display_mut(), slider.clone()));
    picture_widget.set_height(Length::Stretch { min: 0.0, max: f32::INFINITY });
    picture_widget.set_width(Length::Stretch { min: 0.0, max: f32::INFINITY });
    
    bottom_container.add_child(theme_button.clone());
    bottom_container.add_child(slider.clone());
    bottom_container.add_child(help_button.clone());
    
    picture_area_container.add_child(picture_widget.clone());
    picture_area_container.add_child(help_screen.clone());

    vertical_container.add_child(picture_area_container);
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
    let picture_widget_clone = picture_widget.clone();
    let bottom_container_clone = bottom_container.clone();
    let slider_clone = slider.clone();
    let window_clone = window.clone();
    let light_theme = Cell::new(true);
    set_theme(
        light_theme.get(),
        &picture_widget_clone,
        &bottom_container_clone,
        &slider_clone,
        &theme_button_clone,
        &help_button_clone,
        &window_clone,
        &moon,
        &light,
        &question,
        &question_light
    );
    theme_button.set_on_click(move || {
        light_theme.set(!light_theme.get());
        set_theme(
            light_theme.get(),
            &picture_widget_clone,
            &bottom_container_clone,
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
    let image_widget_clone = picture_widget.clone();
    slider.set_on_value_change(move || {
        image_widget_clone.jump_to_index(slider_clone2.value());
    });
    let help_visible = Cell::new(false);
    help_screen.set_visible(help_visible.get());
    help_button.set_on_click(move || {
        help_visible.set(!help_visible.get());
        help_screen.set_visible(help_visible.get());
    });
    window.set_root(vertical_container);
    application.start_event_loop();
}
// ========================================================

fn set_theme(
    light_theme: bool,
    picture_widget: &Rc<PictureWidget>,
    bottom_container: &Rc<HorizontalLayoutContainer>,
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
        picture_widget.set_bright_shade(0.96);
        bottom_container.set_bg_color([1.0, 1.0, 1.0, 1.0]);
        slider.set_shadow_color([0.0, 0.0, 0.0]);
        window.set_bg_color([0.85, 0.85, 0.85, 1.0]);
        theme_button.set_icon(Some(moon_texture.clone()));
        help_button.set_icon(Some(question_texture.clone()));
    } else {
        picture_widget.set_bright_shade(0.3);
        bottom_container.set_bg_color([0.1, 0.1, 0.1, 0.1]);
        slider.set_shadow_color([0.0, 0.0, 0.0]);
        window.set_bg_color([0.05, 0.05, 0.05, 1.0]);
        theme_button.set_icon(Some(light_texture.clone()));
        help_button.set_icon(Some(question_texture_light.clone()));
    }
}
