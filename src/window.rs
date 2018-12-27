use std;

use glium;
use glium::glutin;
use glium::glutin::dpi::{LogicalPosition, LogicalSize};

use configuration::Configuration;

pub struct Window {
    display: glium::Display,
    fullscreen: bool,
}

impl Window {
    pub fn new(events_loop: &glutin::EventsLoop, config: &Configuration) -> Self {
        use glium::glutin::Icon;
        use glium::glutin::MouseCursor;

        let exe_parent = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_owned();

        let icon_path = exe_parent.join("resource/emulsion48.png");
        let icon = Icon::from_path(icon_path.clone()).unwrap_or_else(|_| {
            panic!(format!(
                "Could not load icon '{}'",
                icon_path.to_str().unwrap()
            ))
        });

        let window = glutin::WindowBuilder::new()
            .with_title("Loading")
            .with_fullscreen(None)
            .with_dimensions(LogicalSize::new(
                config.window_width as f64,
                config.window_height as f64,
            ))
            .with_window_icon(Some(icon))
            .with_visibility(false);

        let context = glutin::ContextBuilder::new().with_gl_profile(glutin::GlProfile::Core);
        let display = glium::Display::new(window, context, events_loop).unwrap();

        display.gl_window().set_position(LogicalPosition::new(
            config.window_x as f64,
            config.window_y as f64,
        ));
        display.gl_window().show();
        display.gl_window().set_cursor(MouseCursor::Default);

        let resulting_window = Window {
            display,
            fullscreen: false
        };

        resulting_window
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        let window = self.display.gl_window();
        let monitor = if fullscreen {
            Some(window.get_current_monitor())
        } else {
            None
        };
        window.set_fullscreen(monitor);
    }

    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    pub fn display<'a>(&'a self) -> &'a glium::Display {
        &self.display
    }

    pub fn set_title_filename(&mut self, name: &str) {
        self.display
            .gl_window()
            .set_title(Self::create_title_filename(name).as_ref());
    }

    fn create_title_filename(name: &str) -> String {
        // Separator character used to be ⬕
        // But that one does not display correctly on Ubuntu 18.04

        format!("E M U L S I O N : {}", name)
    }
}
