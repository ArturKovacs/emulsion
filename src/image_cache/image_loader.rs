use std;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;

use glium;

use glium::texture::{RawImage2d, SrgbTexture2d};

use image;

pub mod errors {
    use glium::texture;
    use image;
    use std::io;

    error_chain!{
        foreign_links {
            Io(io::Error) #[doc = "Error during IO"];
            TextureCreationError(texture::TextureCreationError);
            ImageLoadError(image::ImageError);
        }
    }
}

use self::errors::*;

pub fn load_image(image_path: &Path) -> Result<image::RgbaImage> {
    Ok(image::open(image_path)?.to_rgba())
}

pub fn texture_from_image(
    display: &glium::Display,
    image: image::RgbaImage,
) -> Result<SrgbTexture2d> {
    let image_dimensions = image.dimensions();
    let image = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);

    let mipmaps = if image_dimensions.0 == 1 || image_dimensions.1 == 1 {
        glium::texture::MipmapsOption::NoMipmap
    } else {
        glium::texture::MipmapsOption::AutoGeneratedMipmapsMax(4)
    };

    Ok(SrgbTexture2d::with_mipmaps(display, image, mipmaps)?)
}

pub fn get_image_size_estimate(dimensions: (u32, u32)) -> u32 {
    // counting all the mipmaps would add an additionnal multiplier of around ~1.6
    // but only the gpu textures have mip maps so just multiply by 1.5
    ((dimensions.0 * dimensions.1 * 4) as f32 * 1.5) as u32
}

pub fn is_file_supported(filename: &Path) -> bool {
    if let Some(ext) = filename.extension() {
        if let Some(ext) = ext.to_str() {
            let ext = ext.to_lowercase();
            match ext.as_str() {
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "tif" | "tiff" | "tga" | "bmp"
                | "ico" | "hdr" | "pbm" | "pam" | "ppm" | "pgm" => {
                    return true;
                }
                _ => (),
            }
        }
    }

    false
}

pub enum CachedTexture {
    Texture((fs::Metadata, Rc<SrgbTexture2d>)),
    LoadRequested,
}

pub enum LoadResult {
    Ok {
        path: PathBuf,
        metadata: fs::Metadata,
        image: image::RgbaImage,
    },
    Failed,
}

pub struct ImageLoader {
    running: Arc<AtomicBool>,
    join_handles: Option<Vec<thread::JoinHandle<()>>>,
    image_rx: Receiver<LoadResult>,
    path_tx: Sender<PathBuf>,
}

impl ImageLoader {
    /// # Arguemnts
    /// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
    pub fn new(threads: u32) -> ImageLoader {
        let running = Arc::new(AtomicBool::from(true));
        //let loader_cache = HashMap::new();

        let (load_request_tx, load_request_rx) = channel();
        let load_request_rx = Arc::new(Mutex::new(load_request_rx));

        let (loaded_img_tx, loaded_img_rx) = channel();

        let mut join_handles = Vec::new();
        for _ in 0..threads {
            let mut running = running.clone();
            let mut load_request_rx = load_request_rx.clone();
            let mut loaded_img_tx = loaded_img_tx.clone();

            join_handles.push(thread::spawn(move || {
                Self::thread_loop(running, load_request_rx, loaded_img_tx);
            }));
        }

        ImageLoader {
            //curr_dir: PathBuf::new(),
            //curr_est_size: capacity as usize,
            running,
            //remaining_capacity: capacity,
            //total_capacity: capacity,
            //loader_cache,
            //texture_cache: BTreeMap::new(),
            join_handles: Some(join_handles),

            image_rx: loaded_img_rx,
            path_tx: load_request_tx,
            //requested_images: 0,
        }
    }

    fn thread_loop(
        running: Arc<AtomicBool>,
        load_request_rx: Arc<Mutex<Receiver<PathBuf>>>,
        loaded_img_tx: Sender<LoadResult>,
    ) {
        // walk the directory starting from the current item and cache in all the images
        // do this by stepping in both directions so that the cached images ahead of the file
        // should never be more than 1 + "cached images before the file"
        while running.load(Ordering::Acquire) {
            let img_path = {
                let load_request = load_request_rx.lock().unwrap();
                load_request.recv().unwrap()
            };
            // It is very important that we release the mutex before starting to load the image

            let result = {
                if let Ok(metadata) = fs::metadata(img_path.as_path()) {
                    if let Ok(image) = load_image(img_path.as_path()) {
                        LoadResult::Ok {
                            path: img_path,
                            metadata,
                            image,
                        }
                    } else {
                        LoadResult::Failed
                    }
                } else {
                    LoadResult::Failed
                }
            };

            loaded_img_tx.send(result).unwrap();
        }
    }

    pub fn try_recv_prefetched(&mut self) -> std::result::Result<LoadResult, TryRecvError> {
        self.image_rx.try_recv()
    }

    pub fn send_load_request(&mut self, path: PathBuf) {
        self.path_tx.send(path).unwrap();
    }
}

impl Drop for ImageLoader {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);

        match self.join_handles.take() {
            Some(join_handles) => {
                for _ in join_handles.iter() {
                    self.path_tx.send(PathBuf::from("")).unwrap();
                }

                for mut handle in join_handles.into_iter() {
                    match handle.join() {
                        Err(err) => eprintln!("Error occured while joining handle {:?}", err),
                        _ => (),
                    }
                }
            }
            _ => (),
        }
    }
}
