use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::mem;

use glium;

use glium::texture::{RawImage2d, SrgbTexture2d};

use image;

pub mod errors {
    use std::io;
    use glium::texture;
    use image;

    error_chain!{
        foreign_links {
            Io(io::Error) #[doc = "Error during IO"];
            TextureCreationError(texture::TextureCreationError);
            ImageError(image::ImageError);
        }
    }
}


use self::errors::*;


enum LoaderImage {
    Image(image::RgbaImage),
    Processed
}


pub struct ImageCache {
    dir_path: PathBuf,

    running: Arc<AtomicBool>,
    remaining_capacity: Arc<AtomicUsize>,
    loader_cache: Arc<Mutex<HashMap<PathBuf, (fs::Metadata, LoaderImage)>>>,
    texture_cache: HashMap<PathBuf, (fs::Metadata, Rc<SrgbTexture2d>)>,
    join_handle: Option<thread::JoinHandle<()>>,
}


/// This is a store for the supported images loaded from a folder
/// The basic idea is to have a few images already in the memory while an image is shown on the screen
impl ImageCache {
    /// # Arguemnts
    /// * `capacity` - Kilobytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
    pub fn new(capacity: usize) -> ImageCache {
        let mut running = Arc::new(AtomicBool::from(true));
        let mut remaining_capacity = Arc::new(AtomicUsize::from(capacity));
        let mut loader_cache = Arc::new(Mutex::new(HashMap::new()));

        let mut join_handle = Some({
            let mut running = running.clone();
            let mut remaining_capacity = remaining_capacity.clone();
            let mut cache = loader_cache.clone();
            thread::spawn(move || {
                Self::thread_loop(running, remaining_capacity, cache);
            })
        });

        ImageCache {
            dir_path: PathBuf::new(),
            running,
            remaining_capacity,
            loader_cache,
            texture_cache: HashMap::new(),
            join_handle,
        }
    }


    pub fn load_specific(
        &mut self,
        display: &glium::Display,
        path: &str,
    ) -> Result<Rc<SrgbTexture2d>> {
        use std::collections::hash_map::Entry;

        let path = Path::new(path).canonicalize()?;
        let metadata = fs::metadata(path.as_path())?;

        // Check if it is inside the texture cache first
        let texture_entry = self.texture_cache.entry(path.clone());
        if let Entry::Occupied(ref entry) = texture_entry {
            if entry.get().0.modified().unwrap() == metadata.modified().unwrap() {
                return Ok(entry.get().1.clone());
            }
        }

        // requesting exclusive access to the map for the entire scope to save mayself from looking up the entry twice.
        let mut loader_cache = self.loader_cache.lock().unwrap();
        let mut loader_entry = (*loader_cache).entry(path.clone());
        if let Entry::Occupied(ref mut entry) = loader_entry {
            if entry.get().0.modified().unwrap() == metadata.modified().unwrap() {
                // Perform conversion from
                let mut processed_image = LoaderImage::Processed;
                mem::swap(&mut entry.get_mut().1, &mut processed_image);
                let texture = Rc::new(Self::texture_from_loader(display, processed_image)?);
                match texture_entry {
                    Entry::Vacant(entry) => {
                        entry.insert((metadata, texture.clone()));
                    },
                    _ => unreachable!(),
                }
                entry.get_mut().1 = LoaderImage::Processed;
                return Ok(texture);
            }
        };
        // If it wasn't in any if the caches the parent directory may have changed...
        self.dir_path = path.parent().unwrap().to_owned(); // It absolutely must have a parent if it was a file

        let image = Self::load_image(path.as_path())?;
        let result_texture = Rc::new((Self::texture_from_image(display, image)?));
        match texture_entry {
            Entry::Vacant(entry) => {
                entry.insert((metadata.clone(), result_texture.clone()));
            },
            _ => unreachable!(),
        }
        match loader_entry {
            Entry::Vacant(entry) => {
                entry.insert((metadata, LoaderImage::Processed));
            }
            _ => unreachable!(),
        };

        Ok(result_texture)
    }


    fn load_image(image_path: &Path) -> Result<image::RgbaImage> {
        Ok(image::open(image_path)?.to_rgba())
    }


    fn thread_loop(
        running: Arc<AtomicBool>,
        remaining_capacity: Arc<AtomicUsize>,
        cache: Arc<Mutex<HashMap<PathBuf, (fs::Metadata, LoaderImage)>>>,
    ) {
        // walk the directory starting from the current item and cache in all the images
        // do this by stepping in both directions so that the cached images ahead of the file
        // should never be more than 1 + "cached images before the file"
        while running.load(Ordering::SeqCst) {

        }
    }

    fn texture_from_loader(display: &glium::Display, image: LoaderImage) -> Result<SrgbTexture2d> {
        match image {
            LoaderImage::Image(image) => {
                let result = Self::texture_from_image(display, image)?;
                Ok(result)
            },
            LoaderImage::Processed => {
                Err(Error::from("Loader image was requested to be converted but it has already been processed"))
            }
        }
    }

    fn texture_from_image(display: &glium::Display, image: image::RgbaImage) -> Result<SrgbTexture2d> {
        let image_dimensions = image.dimensions();
        let image = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);

        Ok(SrgbTexture2d::with_mipmaps(
            display,
            image,
            glium::texture::MipmapsOption::AutoGeneratedMipmapsMax(4),
        )?)
    }

}


impl Drop for ImageCache {
    fn drop(&mut self) {
        self.running.store(false,Ordering::SeqCst);
        self.join_handle.take().unwrap().join().unwrap();
    }
}
