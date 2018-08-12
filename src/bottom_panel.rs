use std::cell::RefCell;
use std::env;
use std::path::Path;
use std::rc::Rc;

use glium;
use glium::glutin;
use glium::glutin::WindowEvent;
use glium::texture::{RawImage2d, SrgbTexture2d};

use image;

use cgmath::{Vector2, Vector3};

use configuration::Configuration;
use playback_manager::{LoadRequest, PlaybackManager};
use ui::Ui;
use ui::slider::Slider;
use ui::toggle::Toggle;
use window::*;

fn load_texture_without_cache(display: &glium::Display, image_path: &Path) -> SrgbTexture2d {
    let image = image::open(image_path).unwrap().to_rgba();

    texture_from_image(display, image)
}

fn texture_from_image(display: &glium::Display, image: image::RgbaImage) -> SrgbTexture2d {
    let image_dimensions = image.dimensions();
    let image = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);

    SrgbTexture2d::with_mipmaps(display, image, glium::texture::MipmapsOption::NoMipmap).unwrap()
}


fn set_theme<'callback_ref>(
    light_theme: bool,
    slider: &Rc<RefCell<Slider<'callback_ref>>>,
    theme_toggle: &Rc<RefCell<Toggle<'callback_ref>>>,
    help_toggle: &Rc<RefCell<Toggle<'callback_ref>>>,
) {
    let shadow_color =  if light_theme {
        Vector3::new(0.0, 0.0, 0.0f32)
    } else {
        Vector3::new(0.6, 0.6, 0.6f32)
    };

    let mut slider = slider.borrow_mut();
    let mut theme_toggle = theme_toggle.borrow_mut();
    let mut help_toggle = help_toggle.borrow_mut();
    slider.set_shadow_color(shadow_color);
    theme_toggle.set_shadow_color(shadow_color);
    help_toggle.set_shadow_color(shadow_color);
}


pub struct BottomPanel<'callback_ref> {
    ui: Ui<'callback_ref>,
    slider: Rc<RefCell<Slider<'callback_ref>>>,
    theme_toggle: Rc<RefCell<Toggle<'callback_ref>>>,
    help_toggle: Rc<RefCell<Toggle<'callback_ref>>>,
}

impl<'callback_ref> BottomPanel<'callback_ref> {
    pub const HEIGHT: i32 = 32;
    pub const CONTROLS_MAX_WIDTH: i32 = 1024;

    pub fn new(
        window: &mut Window,
        playback_manager: &'callback_ref RefCell<PlaybackManager>,
        configuration: &'callback_ref RefCell<Configuration>,
    ) -> Self {
        let mut ui = Ui::new(window.display(), Self::HEIGHT as f32);

        let exe_parent = env::current_exe().unwrap().parent().unwrap().to_owned();
        let light_texture = Rc::new(load_texture_without_cache(
            window.display(),
            &exe_parent.join("light.png"),
        ));
        let moon_texture = Rc::new(load_texture_without_cache(
            window.display(),
            &exe_parent.join("moon.png"),
        ));
        let question = Rc::new(load_texture_without_cache(
            window.display(),
            &exe_parent.join("question_button.png"),
        ));
        let question_light = Rc::new(load_texture_without_cache(
            window.display(),
            &exe_parent.join("question_button_light.png"),
        ));

        let config = configuration.borrow();

        let slider = ui.create_slider(
            Vector2::new(64f32, 3f32),
            Vector2::new(512f32, 24f32),
            32,
            5,
            move |_, value| {
                playback_manager
                    .borrow_mut()
                    .request_load(LoadRequest::LoadAtIndex(value as usize));
            },
        );
        let help_toggle = ui.create_toggle(
            question.clone(),
            question,
            Vector2::new(32f32, 4f32),
            false,
            move |is_on| {
                //configuration.borrow_mut().light_theme = is_light;
            },
        );

        let theme_toggle =  ui.create_toggle(
            moon_texture,
            light_texture,
            Vector2::new(32f32, 4f32),
            config.light_theme,
            |_| {},
        );

        {
            let theme_toggle_clone = theme_toggle.clone();
            let slider_clone = slider.clone();
            let help_toggle_clone = help_toggle.clone();
            theme_toggle.borrow_mut().set_callback(move |is_light| {
                configuration.borrow_mut().light_theme = is_light;
                set_theme(
                    is_light,
                    &slider_clone,
                    &theme_toggle_clone,
                    &help_toggle_clone
                );
            });
        }

        set_theme(
            config.light_theme,
            &slider,
            &theme_toggle,
            &help_toggle
        );

        BottomPanel { ui, slider, theme_toggle, help_toggle }
    }

    pub fn handle_event(&mut self, event: &glutin::Event, window: &Window) {
        use glutin::Event;
        if let Event::WindowEvent { ref event, .. } = event {
            let window_size = window.display().gl_window().get_inner_size().unwrap();
            self.ui.window_event(event, window_size);

            if let WindowEvent::Resized(..) = event {
                const MARGIN: i32 = 32;
                const SPACING: i32 = 32;
                const PADDING: i32 = 4;
                const BUTTON_SIZE: i32 = 24;
                let controls_width = (window_size.width as i32 - MARGIN*2).max(1).min(Self::CONTROLS_MAX_WIDTH);

                let mut x = window_size.width as i32 / 2 - controls_width / 2;

                {
                    let mut toggle = self.theme_toggle.borrow_mut();
                    let pos = toggle.position();
                    toggle.set_position(Vector2::new((x + PADDING) as f32, pos.y));
                    x += 32 + SPACING;
                }


                {
                    let mut slider = self.slider.borrow_mut();
                    let pos = slider.position();
                    slider.set_position(Vector2::new((x + PADDING) as f32, pos.y));
                    let pos = slider.position();
                    let button_space = BUTTON_SIZE + PADDING*2;
                    let width = (controls_width - button_space*2 - SPACING*2 - PADDING*2).max(1);
                    slider.set_size(Vector2::new(width as f32, 24f32));
                    x += width + 8;
                }

                {
                    let mut toggle = self.help_toggle.borrow_mut();
                    let pos = toggle.position();
                    toggle.set_position(Vector2::new((x + 4 + SPACING) as f32, pos.y));
                }
            }
        }
    }

    pub fn draw(
        &mut self,
        target: &mut glium::Frame,
        playback_manager: &PlaybackManager,
        config: &Configuration,
    ) {
        let curr_file_index = playback_manager.current_file_index() as u32;
        let curr_dir_len = playback_manager.current_dir_len() as u32;
        self.slider.borrow_mut().set_steps(curr_dir_len, curr_file_index);
        let color = if config.light_theme {
            [0.95, 0.95, 0.95, 1.0f32]
        } else {
            [0.05, 0.05, 0.05, 1.0f32]
        };

        self.ui.draw(target, &color);
    }
}
