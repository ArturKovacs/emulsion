use std::ffi::OsString;
use std::io::Write;
use std::mem;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Instant, Duration};

use rand::{thread_rng, Rng};

use sys_info;

use gelatin::glium;

use gelatin::window::Window;
//use crate::window::Window;

use crate::image_cache;
use crate::image_cache::ImageCache;

#[derive(PartialEq)]
pub enum LoadRequest {
    None,
    LoadNext,
    LoadPrevious,
    FilePath(PathBuf),
    LoadAtIndex(usize),
    Jump(i32),
}

#[derive(PartialEq, Copy, Clone)]
pub enum PlaybackState {
    Paused,
    Forward,
    Present,
    RandomPresent,
    //Backward,
}

pub struct PlaybackManager {
    playback_state: PlaybackState,
    image_cache: ImageCache,

    present_remaining: Vec<usize>,

    playback_start_time: Instant,
    frame_count_since_playback_start: u64,

    load_request: LoadRequest,

    //should_sleep: bool,

    image_texture: Option<Rc<glium::texture::SrgbTexture2d>>,
    filename: Option<OsString>,
}

impl PlaybackManager {
    pub fn new() -> Self {
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

        let resulting_window = PlaybackManager {
            playback_state: PlaybackState::Paused,
            image_cache: ImageCache::new(cache_capaxity, thread_count),

            present_remaining: Vec::new(),

            playback_start_time: Instant::now(),
            frame_count_since_playback_start: 0,
            load_request: LoadRequest::None,
            //should_sleep: true,

            image_texture: None,
            filename: None,
        };

        resulting_window
    }

    pub fn playback_state(&self) -> PlaybackState {
        self.playback_state
    }

    pub fn start_playback_forward(&mut self) {
        self.playback_start_time = Instant::now();
        self.frame_count_since_playback_start = 0;
        self.playback_state = PlaybackState::Forward;
    }

    pub fn pause_playback(&mut self) {
        self.playback_state = PlaybackState::Paused;
    }

    pub fn start_random_presentation(&mut self) {
        self.playback_start_time = Instant::now();
        self.frame_count_since_playback_start = 0;
        self.playback_state = PlaybackState::RandomPresent;
        self.fill_present_remainig_with_random();
    }

    pub fn start_presentation(&mut self) {
        self.playback_start_time = Instant::now();
        self.frame_count_since_playback_start = 0;
        self.playback_state = PlaybackState::Present;
    }

    pub fn current_filename(&self) -> OsString {
        self.image_cache.current_filename()
    }

    pub fn current_file_path(&self) -> PathBuf {
        self.image_cache.current_file_path()
    }

    pub fn current_file_index(&self) -> usize {
        self.image_cache.current_file_index()
    }

    pub fn current_dir_len(&self) -> usize {
        self.image_cache.current_dir_len()
    }

    pub fn update_directory(&mut self) -> image_cache::Result<()> {
        self.image_cache.update_directory()
    }

    pub fn cached_from_dir(&self) -> Vec<bool> {
        self.image_cache.cached_from_dir()
    }

    pub fn should_sleep(&self) -> bool {
        //self.should_sleep
        false
    }

    pub fn request_load(&mut self, request: LoadRequest) {
        self.load_request = request;
    }

    pub fn curr_load_request<'a>(&'a self) -> &'a LoadRequest {
        &self.load_request
    }

    pub fn image_texture<'a>(&'a self) -> &'a Option<Rc<glium::texture::SrgbTexture2d>> {
        &self.image_texture
    }

    pub fn filename(&self) -> &Option<OsString> {
        &self.filename
    }

    pub fn update_image(&mut self, window: &Window) -> gelatin::NextUpdate {
        //self.should_sleep = true;
        let now = Instant::now();
        let mut next_update;
        // The reason why I reset the load request in such a convoluted way is that
        // it has to guaranteprefetch_neighborsequest will be reset even if I return from this
        // function early
        let mut load_request = LoadRequest::None;
        mem::swap(&mut self.load_request, &mut load_request);

        let framerate = match self.playback_state {
            PlaybackState::Present | PlaybackState::RandomPresent => 0.1667, // six seconds per img
            _ => 25.0,
        };
        const NANOS_PER_SEC: u64 = 1000_000_000;
        let frame_delta_time_nanos = (NANOS_PER_SEC as f64 / framerate) as u64;

        if self.playback_state == PlaybackState::Paused {
            self.image_cache
                .process_prefetched(&window.display_mut())
                .unwrap();
            self.image_cache.prefetch_neighbors();
            next_update = gelatin::NextUpdate::Latest;
        } else {
            if load_request == LoadRequest::None {
                let elapsed = self.playback_start_time.elapsed();
                let elapsed_nanos = elapsed.as_secs() * NANOS_PER_SEC + elapsed.subsec_nanos() as u64;

                let nanos_til_next = frame_delta_time_nanos - (elapsed_nanos % frame_delta_time_nanos);
                let millis_til_next = nanos_til_next / 1000_000;
                next_update = gelatin::NextUpdate::WaitUntil(
                    now.checked_add(
                        Duration::from_millis((millis_til_next / 2).max(1))
                    ).unwrap()
                );
                let frame_step =
                    (elapsed_nanos / frame_delta_time_nanos) - self.frame_count_since_playback_start;
                if frame_step > 0 {
                    load_request = match self.playback_state {
                        PlaybackState::Forward | PlaybackState::Present => {
                            LoadRequest::Jump(frame_step as i32)
                        }
                        //PlaybackState::Backward => LoadRequest::Jump(-(frame_step as i32)),
                        PlaybackState::RandomPresent => {
                            let mut target = None;
                            for _ in 0..frame_step {
                                target = self.present_remaining.pop();
                                if target == None {
                                    // Restart
                                    self.fill_present_remainig_with_random();
                                    target = self.present_remaining.pop();
                                }
                            }
                            match target {
                                Some(index) => LoadRequest::LoadAtIndex(index),
                                None => LoadRequest::None,
                            }
                        }
                        PlaybackState::Paused => unreachable!(),
                    };
                    self.frame_count_since_playback_start += frame_step;
                } else {
                    self.image_cache
                        .process_prefetched(&window.display_mut())
                        .unwrap();

                    let nanos_since_last = elapsed_nanos % frame_delta_time_nanos;
                    const BUISY_WAIT_TRESHOLD: f32 = 0.8;
                    if nanos_since_last > (frame_delta_time_nanos as f32 * BUISY_WAIT_TRESHOLD) as u64 {
                        // Just buisy wait if we are getting very close to the next frame swap
                        next_update = gelatin::NextUpdate::Soonest;
                    } else {
                        match self.playback_state {
                            PlaybackState::RandomPresent => {
                                if let Some(&last) = self.present_remaining.iter().last() {
                                    self.image_cache.prefetch_at_index(last);
                                }
                            }
                            _ => self.image_cache.prefetch_neighbors(),
                        }
                    }
                }
            } else {
                next_update = gelatin::NextUpdate::WaitUntil(
                    now.checked_add(Duration::from_millis(1)).unwrap()
                );
            }
        }

        //let should_sleep = load_request == LoadRequest::None && running && !update_screen;
        // Process long operations here
        let load_result = match load_request {
            LoadRequest::LoadNext => Some(self.image_cache.load_next(&window.display_mut())),
            LoadRequest::LoadPrevious => Some(self.image_cache.load_prev(&window.display_mut())),
            LoadRequest::FilePath(ref file_path) => {
                Some(if let Some(file_name) = file_path.file_name() {
                    self.image_cache
                        .load_specific(&window.display_mut(), file_path.as_ref())
                        .map(|x| (x, OsString::from(file_name)))
                } else {
                    Err(String::from("Could not extract filename").into())
                })
            }
            LoadRequest::LoadAtIndex(index) => {
                Some(self.image_cache.load_at_index(&window.display_mut(), index))
            }
            LoadRequest::Jump(jump_count) => {
                Some(self.image_cache.load_jump(&window.display_mut(), jump_count))
            }
            LoadRequest::None => None,
        };
        if let Some(result) = load_result {
            match result {
                Ok((texture, filename)) => {
                    self.image_texture = Some(texture);
                    self.filename = Some(filename);
                }
                Err(err) => {
                    self.image_texture = None;
                    self.filename = None;
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
            next_update = gelatin::NextUpdate::Soonest;
        }
        next_update
    }

    fn fill_present_remainig_with_random(&mut self) {
        self.present_remaining.clear();
        for i in 0..self.image_cache.current_dir_len() {
            self.present_remaining.push(i);
        }
        thread_rng().shuffle(self.present_remaining.as_mut_slice());
    }
}
