use std::fs;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::thread;
use std::time;
use std::mem;
use std::ffi::OsString;

use std::iter;

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
            ImageLoadError(image::ImageError);
        }
    }
}

use self::errors::*;

enum LoaderImage {
    Image(image::RgbaImage),
    Processed,
}

pub struct ImageCache {
    dir_path: PathBuf,
    current_name: OsString,

    running: Arc<AtomicBool>,
    remaining_capacity: Arc<AtomicIsize>,
    loader_cache: Arc<Mutex<HashMap<PathBuf, (fs::Metadata, LoaderImage)>>>,
    texture_cache: HashMap<PathBuf, (fs::Metadata, Rc<SrgbTexture2d>)>,
    join_handle: Option<thread::JoinHandle<()>>,
}

/// This is a store for the supported images loaded from a folder
/// The basic idea is to have a few images already in the memory while an image is shown on the screen
impl ImageCache {
    /// # Arguemnts
    /// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
    pub fn new(capacity: isize) -> ImageCache {
        let running = Arc::new(AtomicBool::from(true));
        let remaining_capacity = Arc::new(AtomicIsize::from(capacity));
        let loader_cache = Arc::new(Mutex::new(HashMap::new()));

        let join_handle = Some({
            let mut running = running.clone();
            let mut remaining_capacity = remaining_capacity.clone();
            let mut cache = loader_cache.clone();
            thread::spawn(move || {
                Self::thread_loop(running, remaining_capacity, cache);
            })
        });

        ImageCache {
            dir_path: PathBuf::new(),
            current_name: OsString::new(),

            running,
            remaining_capacity,
            loader_cache,
            texture_cache: HashMap::new(),
            join_handle,
        }
    }

    fn thread_loop(
        running: Arc<AtomicBool>,
        remaining_capacity: Arc<AtomicIsize>,
        cache: Arc<Mutex<HashMap<PathBuf, (fs::Metadata, LoaderImage)>>>,
    ) {
        // walk the directory starting from the current item and cache in all the images
        // do this by stepping in both directions so that the cached images ahead of the file
        // should never be more than 1 + "cached images before the file"
        while running.load(Ordering::SeqCst) {
            //thread::yield_now();
            thread::sleep(time::Duration::from_millis(1));
        }
    }

    //
    pub fn load_specific(
        &mut self,
        display: &glium::Display,
        path: &str,
    ) -> Result<Rc<SrgbTexture2d>> {
        use std::collections::hash_map::Entry;

        let path = Path::new(path).canonicalize()?;
        let metadata = fs::metadata(path.as_path())?;

        self.current_name = match path.file_name() {
            Some(filename) => filename.to_owned(),
            None => bail!(format!("Could not get filename for path '{}'", path.to_str().unwrap())),
        };

        let mut loader_cache;
        // Check if it is inside the texture cache first
        {
            let texture_entry = self.texture_cache.entry(path.clone());
            if let Entry::Occupied(ref entry) = texture_entry {
                if entry.get().0.modified().unwrap() == metadata.modified().unwrap() {
                    return Ok(entry.get().1.clone());
                }
            }

            // requesting exclusive access to the map for the entire scope to save mayself from looking up the entry twice.
            loader_cache = self.loader_cache.lock().unwrap();
            if let Entry::Occupied(ref mut entry) = (*loader_cache).entry(path.clone()) {
                if entry.get().0.modified().unwrap() == metadata.modified().unwrap() {
                    // Perform conversion from
                    let mut processed_image = LoaderImage::Processed;
                    mem::swap(&mut entry.get_mut().1, &mut processed_image);
                    let texture = Rc::new(Self::texture_from_loader(display, processed_image)?);
                    match texture_entry {
                        Entry::Vacant(entry) => {
                            entry.insert((metadata, texture.clone()));
                        }
                        _ => unreachable!(),
                    }
                    entry.get_mut().1 = LoaderImage::Processed;
                    return Ok(texture);
                }
            }
        }

        // If it wasn't in any if the caches the parent directory may have changed...
        self.dir_path = path.parent().unwrap().to_owned(); // It absolutely must have a parent if it was a file

        let image = Self::load_image(path.as_path())?;
        let image_size_estimate = Self::get_image_size_estimate((image.width(), image.height())) as isize;
        if self.remaining_capacity.load(Ordering::SeqCst) < image_size_estimate {
            // Empty the files furthest from this current one
            // Collect all the files first in the order they are returned from the OS call
            let dir_files: Vec<_> = fs::read_dir(self.dir_path.as_path())?
                .filter_map(|x| {
                    let entry = x.unwrap();
                    if entry.file_type().unwrap().is_file() {
                        Some(entry)
                    } else {
                        None
                    }
                }).collect();

            // Find the position of the current file in the directory
            let mut current_pos = 0;
            let loaded_filename = path.file_name().unwrap();
            for (i, file_in_dir) in dir_files.iter().enumerate() {
                if file_in_dir.file_name() == loaded_filename {
                    current_pos = i as i32;
                }
            }

            let mut cached_ordered: Vec<_> = dir_files.iter().filter_map(|file| {
                let path = file.path();
                if self.texture_cache.contains_key(&path) || (*loader_cache).contains_key(&path) {
                    Some(path)
                } else {
                    None
                }
            }).enumerate().collect();

            cached_ordered.sort_unstable_by(|&(pos_a, _), &(pos_b, _)| {
                let a_dist = (pos_a as i32 - current_pos).abs();
                let b_dist = (pos_b as i32 - current_pos).abs();
                // sort in decreasing order
                b_dist.cmp(&a_dist)
            });

            // And there is just one more thing left to do...
            // Walk through our list of directory entries sorted by their distance from the current
            // file and in each step remove an entry from the cache until we reach the desired cache
            // size
            for (_, file) in cached_ordered.into_iter() {
                if self.remaining_capacity.load(Ordering::SeqCst) < image_size_estimate {
                    let mut loader_entry = (*loader_cache).entry(file.clone());
                    if let Entry::Occupied(entry) = loader_entry {
                        let size_estimate = match entry.get().1 {
                            LoaderImage::Image(ref image) => Self::get_image_size_estimate(image.dimensions()) as isize,
                            LoaderImage::Processed => 0
                        };
                        entry.remove();
                        self.remaining_capacity.fetch_add(size_estimate, Ordering::SeqCst);
                    }
                    if self.remaining_capacity.load(Ordering::SeqCst) < image_size_estimate {
                        if let Entry::Occupied(entry) = self.texture_cache.entry(file) {
                            let size_estimate = Self::get_image_size_estimate((entry.get().1.width(), entry.get().1.height())) as isize;
                            entry.remove();
                            self.remaining_capacity.fetch_add(size_estimate, Ordering::SeqCst);
                        }
                    }
                } else {
                    break;
                }
            }
        }
        self.remaining_capacity.fetch_sub(image_size_estimate, Ordering::SeqCst);

        let result_texture = Rc::new(Self::texture_from_image(display, image)?);
        match self.texture_cache.entry(path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert((metadata.clone(), result_texture.clone()));
            }
            _ => unreachable!(),
        }
        match (*loader_cache).entry(path.clone()) {
            Entry::Vacant(entry) => {
                entry.insert((metadata, LoaderImage::Processed));
            }
            _ => unreachable!(),
        };

        Ok(result_texture)
    }

    pub fn load_next(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        let elements: Vec<_> = fs::read_dir(self.dir_path.as_path())
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        self.load_iter_next(display, elements.iter().chain(elements.iter()))
    }

    pub fn load_prev(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        let elements: Vec<_> = fs::read_dir(self.dir_path.as_path())
            .unwrap()
            .map(|x| x.unwrap())
            .collect();
        self.load_iter_next(display, elements.iter().chain(elements.iter()).rev())
    }

    ///
    /// entries_twice should be an iterator of the folder chained with itself.
    ///
    fn load_iter_next<'a, IterT>(
        &mut self,
        display: &glium::Display,
        mut entries_twice: IterT,
    ) -> Result<(Rc<SrgbTexture2d>, OsString)>
    where
        IterT: iter::Iterator<Item = &'a fs::DirEntry>,
    {
        'finding_current: while let Some(curr_file) = entries_twice.next() {
            if curr_file.file_type()?.is_file() {
                if curr_file.file_name() == self.current_name {
                    // Find next file
                    'finding_next: while let Some(next) = entries_twice.next() {
                        if next.file_type().unwrap().is_file() {
                            let next_filename = next.path();
                            match self.load_specific(display, next_filename.to_str().unwrap()) {
                                Err(Error(ErrorKind::ImageLoadError(_err), ..)) => {
                                    // Image type not supported, just skip it
                                    continue 'finding_next;
                                }
                                Err(err) => {
                                    // Some other error occured, it is a bad sign,
                                    // just return the error
                                    return Err(err);
                                }
                                Ok(result) => {
                                    return Ok((
                                        result,
                                        next_filename.file_name().unwrap().to_owned(),
                                    ));
                                }
                            }
                        }
                    }
                    // Current already found at this point
                    break 'finding_current;
                }
            }
        }

        Err(Error::from(
            "Couldn't find the current file in the the directory",
        ))
    }

    fn load_image(image_path: &Path) -> Result<image::RgbaImage> {
        Ok(image::open(image_path)?.to_rgba())
    }

    fn texture_from_loader(display: &glium::Display, image: LoaderImage) -> Result<SrgbTexture2d> {
        match image {
            LoaderImage::Image(image) => {
                let result = Self::texture_from_image(display, image)?;
                Ok(result)
            }
            LoaderImage::Processed => Err(Error::from(
                "Loader image was requested to be converted but it has already been processed",
            )),
        }
    }

    fn texture_from_image(
        display: &glium::Display,
        image: image::RgbaImage,
    ) -> Result<SrgbTexture2d> {
        let image_dimensions = image.dimensions();
        let image = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);

        Ok(SrgbTexture2d::with_mipmaps(
            display,
            image,
            glium::texture::MipmapsOption::AutoGeneratedMipmapsMax(4),
        )?)
    }

    fn get_image_size_estimate(dimensions: (u32, u32)) -> u32 {
        // counting all the mipmaps would add an additionnal multiplier of around ~1.6
        // but only the gpu textures have mip maps so just multiply by 1.5
        ((dimensions.0 * dimensions.1 * 4) as f32 * 1.5) as u32
    }
}

impl Drop for ImageCache {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        self.join_handle.take().unwrap().join().unwrap();
    }
}
