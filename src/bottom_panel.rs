use std::cell::RefCell;
use std::env;
use std::path::Path;
use std::rc::Rc;

use glium;
use glium::glutin;
use glium::glutin::WindowEvent;
use glium::texture::{RawImage2d, SrgbTexture2d};

use image;

use cgmath::Vector2;

use configuration::Configuration;
use playback_manager::{LoadRequest, PlaybackManager};
use ui::{SliderId, Ui};
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

pub struct BottomPanel<'a> {
    ui: Ui<'a>,
    slider: SliderId<'a>,
}

impl<'a> BottomPanel<'a> {
    pub const HEIGHT: u32 = 32;

    pub fn new(
        window: &mut Window,
        playback_manager: &'a RefCell<PlaybackManager>,
        configuration: &'a RefCell<Configuration>,
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

        let config = configuration.borrow();

        let _ = ui.create_toggle(
            moon_texture,
            light_texture,
            Vector2::new(4f32, 4f32),
            config.light_theme,
            Box::new(move |is_light| {
                configuration.borrow_mut().light_theme = is_light;
            }),
        );
        let slider = ui.create_slider(
            Vector2::new(64f32, 3f32),
            Vector2::new(512f32, 24f32),
            32,
            5,
            Box::new(move |_, value| {
                playback_manager
                    .borrow_mut()
                    .request_load(LoadRequest::LoadAtIndex(value as usize));
            }),
        );

        BottomPanel { ui, slider }
    }

    pub fn handle_event(&mut self, event: &glutin::Event, window: &Window) {
        use glutin::Event;
        if let Event::WindowEvent { ref event, .. } = event {
            let window_size = window.display().gl_window().get_inner_size().unwrap();
            self.ui.window_event(event, window_size);

            if let WindowEvent::Resized(..) = event {
                if let Some(slider) = self.ui.get_slider_mut(self.slider) {
                    let pos = slider.position();
                    slider.set_size(Vector2::new(window_size.width as f32 - pos.x - 4f32, 24f32));
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
        self.ui
            .get_slider_mut(self.slider)
            .unwrap()
            .set_steps(curr_dir_len, curr_file_index);
        let color = if config.light_theme {
            [0.95, 0.95, 0.95, 1.0f32]
        } else {
            [0.05, 0.05, 0.05, 1.0f32]
        };
        self.ui.draw(target, &color);
    }
}
