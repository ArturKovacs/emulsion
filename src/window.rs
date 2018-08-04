
use std;

use glium;
use glium::glutin;
use glium::glutin::dpi::LogicalSize;


pub struct Window {
    display: glium::Display,
}

impl Window {
    pub fn init(events_loop: &glutin::EventsLoop) -> Self {
        use glium::glutin::Icon;

        let exe_parent = std::env::current_exe().unwrap().parent().unwrap().to_owned();

        let icon_path = exe_parent.join("emulsion32.png");
        let icon = Icon::from_path(icon_path.clone())
            .unwrap_or_else(|_| panic!(format!("Could not load icon '{}'", icon_path.to_str().unwrap())));

        let window = glutin::WindowBuilder::new()
            .with_title("Loading")
            .with_dimensions(LogicalSize::new(512.0, 512.0))
            .with_fullscreen(None)
            .with_window_icon(Some(icon))
            //.with_decorations(true)
            .with_visibility(true);

        //let context = glutin::ContextBuilder::new().with_gl(GlRequest::Specific(Api::OpenGl, (3, 1)));
        let context = glutin::ContextBuilder::new().with_gl_profile(glutin::GlProfile::Core);
        let display = glium::Display::new(window, context, events_loop).unwrap();

        let resulting_window = Window {
            display,
        };

        resulting_window
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

        format!("E M U L S I O N / {}", name)
    }
}