
use std;
use std::env;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::thread;
use std::time::{Duration, Instant};

use sys_info;
use image;

use glium;
use glium::glutin::{VirtualKeyCode, WindowEvent};
use glium::{glutin, Surface};
use glium::glutin::dpi::LogicalSize;
use glium::texture::{RawImage2d, SrgbTexture2d};

use cgmath::ElementWise;
use cgmath::SquareMatrix;
use cgmath::{Matrix4, Vector2, Vector4};



use ui;
use shaders;

use image_cache::ImageCache;

use picture_controller::PictureController;

#[derive(PartialEq)]
pub enum LoadRequest {
    None,
    LoadNext,
    LoadPrevious,
    LoadSpecific(PathBuf),
    Jump(i32),
}

#[derive(PartialEq)]
pub enum PlaybackState {
    Paused,
    Forward,
    //Backward,
}


pub struct Window {
    pub image_cache: ImageCache,

    pub display: glium::Display,

    pub playback_state: PlaybackState,
    pub playback_start_time: Instant,
    pub frame_count_since_playback_start: u64,

    pub load_request: LoadRequest,

    should_sleep: bool,
    running: bool,

    pub image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
}


impl Window {
    pub const BOTTOM_PANEL_HEIGHT: u32 = 32;

    pub fn init(events_loop: &glutin::EventsLoop) -> Window {
        use glium::glutin::Icon;

        let img_path = env::args().skip(1).next();
        let img_name = match img_path {
            Some(ref img_path) => {
                let img_path = std::path::Path::new(img_path.as_str());
                img_path.file_name().unwrap().to_str()
            }
            _ => None,
        };

        let title = Self::create_title_filename(if let Some(name) = img_name { name } else { "" });

        let exe_parent = std::env::current_exe().unwrap().parent().unwrap().to_owned();

        let icon_path = exe_parent.join("emulsion32.png");
        let icon = Icon::from_path(icon_path.clone())
            .unwrap_or_else(|_| panic!(format!("Could not load icon '{}'", icon_path.to_str().unwrap())));

        let window = glutin::WindowBuilder::new()
            .with_title(title)
            .with_dimensions(LogicalSize::new(512.0, 512.0))
            .with_fullscreen(None)
            .with_window_icon(Some(icon))
            //.with_decorations(true)
            .with_visibility(true);

        //let context = glutin::ContextBuilder::new().with_gl(GlRequest::Specific(Api::OpenGl, (3, 1)));
        let context = glutin::ContextBuilder::new().with_gl_profile(glutin::GlProfile::Core);
        let display = glium::Display::new(window, context, events_loop).unwrap();


        let cache_capaxity = match sys_info::mem_info() {
            Ok(value) => {
                // value originally reported in KiB
                ((value.total / 8) * 1024) as isize
            }
            _ => {
                println!("Could not get system memory size, using default value");
                // bytes
                500_000_000
            }
        };

        let thread_count = match sys_info::cpu_num() {
            Ok(value) => value.max(2).min(4),
            _ => 4,
        };

        let mut resulting_window = Window {
            image_cache: ImageCache::new(cache_capaxity, thread_count),
            display,

            playback_state: PlaybackState::Paused,
            playback_start_time: Instant::now(),
            frame_count_since_playback_start: 0,
            load_request: LoadRequest::None,
            should_sleep: true,
            running: true,

            image_texture: None
        };

        if let Some(img_path) = env::args().skip(1).next() {
            resulting_window.load_image(img_path.as_ref());
        };

        resulting_window
    }


    pub fn should_sleep(&self) -> bool {
        self.should_sleep
    }

    pub fn request_load(&mut self, request: LoadRequest) {
        self.load_request = request;
    }


    pub fn update_playback(&mut self) {
        let framerate = 25.0;
        const NANOS_PER_SEC: u64 = 1000_000_000;
        let frame_delta_time_nanos = (NANOS_PER_SEC as f64 / framerate) as u64;

        if self.playback_state == PlaybackState::Paused {
            self.image_cache.process_prefetched(&self.display).unwrap();
            self.image_cache.send_load_requests();
        } else if self.load_request == LoadRequest::None {
            let elapsed = self.playback_start_time.elapsed();
            let elapsed_nanos =
                elapsed.as_secs() * NANOS_PER_SEC + elapsed.subsec_nanos() as u64;
            let frame_step =
                (elapsed_nanos / frame_delta_time_nanos) - self.frame_count_since_playback_start;
            if frame_step > 0 {
                self.load_request = match self.playback_state {
                    PlaybackState::Forward => LoadRequest::Jump(frame_step as i32),
                    //PlaybackState::Backward => LoadRequest::Jump(-(frame_step as i32)),
                    PlaybackState::Paused => unreachable!(),
                };
                self.frame_count_since_playback_start += frame_step;
            } else {
                self.image_cache.process_prefetched(&self.display).unwrap();

                let nanos_since_last = elapsed_nanos % frame_delta_time_nanos;
                const BUISY_WAIT_TRESHOLD: f32 = 0.8;
                if nanos_since_last
                    > (frame_delta_time_nanos as f32 * BUISY_WAIT_TRESHOLD) as u64
                {
                    // Just buisy wait if we are getting very close to the next frame swap
                    self.should_sleep = false;
                } else {
                    self.image_cache.send_load_requests();
                }
            }
        }

        //let should_sleep = load_request == LoadRequest::None && running && !update_screen;
        // Process long operations here
        let load_result = match self.load_request {
            LoadRequest::LoadNext => Some(self.image_cache.load_next(&self.display)),
            LoadRequest::LoadPrevious => Some(self.image_cache.load_prev(&self.display)),
            LoadRequest::LoadSpecific(ref file_path) => Some(
                if let Some(file_name) = file_path.file_name() {
                    self.image_cache
                        .load_specific(&self.display, file_path.as_ref())
                        .map(|x| (x, OsString::from(file_name)))
                } else {
                    Err(String::from("Could not extract filename").into())
                }
            ),
            LoadRequest::Jump(jump_count) => {
                Some(self.image_cache.load_jump(&self.display, jump_count))
            }
            LoadRequest::None => None,
        };
        if let Some(result) = load_result {
            match result {
                Ok((texture, filename)) => {
                    self.image_texture = Some(texture);
                    // FIXME the program hangs when the title is set during a resize
                    // this is due to the way glutin/winit is architected.
                    // An issu already exists in winit proposing to redesign
                    // the even loop.
                    // Until that is implemented the title is simply not updated during
                    // playback.
                    if self.playback_state == PlaybackState::Paused {
                        self.set_title_filename(filename.to_str().unwrap());
                    }
                }
                Err(err) => {
                    self.image_texture = None;
                    self.set_title_filename("[none]");
                    let stderr = &mut ::std::io::stderr();
                    let stderr_errmsg = "Error writing to stderr";
                    writeln!(stderr, "Error occured while loading image: {}", err)
                        .expect(stderr_errmsg);
                    for e in err.iter().skip(1) {
                        writeln!(stderr, "... caused by: {}", e).expect(stderr_errmsg);
                    }
                    if let Some(backtrace) = err.backtrace() {
                        writeln!(stderr, "backtrace: {:?}", backtrace).expect(stderr_errmsg);
                    }
                    writeln!(stderr).expect(stderr_errmsg);
                }
            }

            self.should_sleep = false;
        }
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

    fn load_image(&mut self, path: &Path) {
        self.image_texture = Some(self.image_cache.load_specific(&self.display, path).unwrap());
    }


    fn load_texture_without_cache(
        display: &glium::Display,
        image_path: &Path,
    ) -> SrgbTexture2d {
        let image = image::open(image_path).unwrap().to_rgba();

        Self::texture_from_image(display, image)
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
}