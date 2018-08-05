

use std::env;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::cell::RefCell;
use std::thread;
use std::time::Duration;

use glium;
use glium::glutin::{VirtualKeyCode, WindowEvent};
use glium::{glutin, Surface};
use glium::texture::{RawImage2d, SrgbTexture2d};

use image;

use cgmath::Vector2;

use ui::{Ui, SliderId};
use picture_panel::PicturePanel;
use playback_manager::{PlaybackManager, LoadRequest};
use window::*;


fn load_texture_without_cache(
    display: &glium::Display,
    image_path: &Path,
) -> SrgbTexture2d {
    let image = image::open(image_path).unwrap().to_rgba();

    texture_from_image(display, image)
}

fn texture_from_image(
    display: &glium::Display,
    image: image::RgbaImage,
) -> SrgbTexture2d {
    let image_dimensions = image.dimensions();
    let image = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);

    SrgbTexture2d::with_mipmaps(
        display,
        image,
        glium::texture::MipmapsOption::NoMipmap,
    ).unwrap()
}


pub struct BottomPanel<'a> {
    ui: Ui<'a>,
    slider: SliderId<'a>
}


impl<'a> BottomPanel<'a> {
    pub const HEIGHT: u32 = 32;

    pub fn new(
        window: &mut Window,
        playback_manager: &'a RefCell<PlaybackManager>,
    ) -> Self {
        let mut ui = Ui::new(window.display());

        let exe_parent = env::current_exe().unwrap().parent().unwrap().to_owned();
        let button_texture = Rc::new(
            load_texture_without_cache(
                window.display(),
                &exe_parent.join("cogs.png")
            )
        );
        let light_texture = Rc::new(
            load_texture_without_cache(
                window.display(),
                &exe_parent.join("light.png")
            )
        );
        let moon_texture = Rc::new(
            load_texture_without_cache(
                window.display(),
                &exe_parent.join("moon.png")
            )
        );

        let button = ui.create_button(button_texture, Vector2::new(32f32, 4f32), Box::new(||()));
        {
            if let Some(button) = ui.get_button_mut(button) {
                button.set_callback(Box::new(move || {
                    playback_manager.borrow_mut().request_load(LoadRequest::LoadNext);
                }));
            }
        }
        let _ = ui.create_toggle(moon_texture, light_texture, Vector2::new(4f32, 4f32), true,
            Box::new(move |_is_light| {
                playback_manager.borrow_mut().request_load(LoadRequest::LoadNext);
            })
        );
        let slider = ui.create_slider(Vector2::new(64f32, 3f32), Vector2::new(512f32, 24f32), 32, 5,
            Box::new(|_, value| {
                println!("Jumped to {}", value);
            })
        );

        BottomPanel {
            ui,
            slider
        }
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


    pub fn draw(&self, target: &mut glium::Frame) {
        self.ui.draw(target);
    }
}
