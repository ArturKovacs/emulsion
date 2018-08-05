use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::mem;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
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

pub enum CachedTexture {
    Texture((fs::Metadata, Rc<SrgbTexture2d>)),
    LoadRequested,
}

pub struct TextureLoader {
    curr_dir: PathBuf,
    curr_est_size: usize,

    running: Arc<AtomicBool>,
    remaining_capacity: isize,
    total_capacity: isize,

    texture_cache: BTreeMap<OsString, CachedTexture>,
    join_handles: Option<Vec<thread::JoinHandle<()>>>,

    image_rx: Receiver<(PathBuf, fs::Metadata, image::RgbaImage)>,
    path_tx: Sender<PathBuf>,
}

impl TextureLoader {
    const MAX_BULK_PREFETCH_REQUEST: i32 = 4;

    /// # Arguemnts
    /// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
    pub fn new(capacity: isize, threads: u32) -> TextureLoader {
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

        TextureLoader {
            curr_dir: PathBuf::new(),
            curr_est_size: capacity as usize,

            running,
            remaining_capacity: capacity,
            total_capacity: capacity,
            //loader_cache,
            texture_cache: BTreeMap::new(),
            join_handles: Some(join_handles),

            image_rx: loaded_img_rx,
            path_tx: load_request_tx,
        }
    }

    fn thread_loop(
        running: Arc<AtomicBool>,
        load_request_rx: Arc<Mutex<Receiver<PathBuf>>>,
        loaded_img_tx: Sender<(PathBuf, fs::Metadata, image::RgbaImage)>,
    ) {
        // walk the directory starting from the current item and cache in all the images
        // do this by stepping in both directions so that the cached images ahead of the file
        // should never be more than 1 + "cached images before the file"
        while running.load(Ordering::Acquire) {
            let img_path = {
                let load_request = load_request_rx.lock().unwrap();
                if let Some(path) = load_request.recv().ok() {
                    path
                } else {
                    return;
                }
            };
            // It is very important that we release the mutex before starting to load the image

            let metadata = match fs::metadata(img_path.as_path()) {
                Ok(metadata) => metadata,
                Err(_) => continue,
            };
            let image = match Self::load_image(img_path.as_path()) {
                Ok(image) => image,
                Err(_) => continue,
            };

            if loaded_img_tx.send((img_path, metadata, image)).is_err() {
                return;
            }
            //thread::sleep(time::Duration::from_millis(1));
        }
    }

    pub fn process_prefetched(&mut self, display: &glium::Display) -> Result<()> {
        use std::collections::btree_map::Entry;
        use std::sync::mpsc::TryRecvError;

        loop {
            match self.image_rx.try_recv() {
                Ok((path, metadata, image)) => {
                    let size_estimate =
                        Self::get_image_size_estimate((image.width(), image.height())) as isize;
                    match self.texture_cache.entry(path.file_name().unwrap().to_owned())
                    {
                        Entry::Vacant(entry) => {
                            let texture = Rc::new(Self::texture_from_image(display, image)?);
                            entry.insert(CachedTexture::Texture((metadata, texture)));
                            self.remaining_capacity -= size_estimate;
                        }
                        Entry::Occupied(mut entry) => match entry.get_mut() {
                            CachedTexture::Texture(ref mut entry) => {
                                if entry.0.modified().unwrap() < metadata.modified().unwrap() {
                                    let old_size_estimate = {
                                        let old_image = &entry.1;
                                        Self::get_image_size_estimate((
                                            old_image.width(),
                                            old_image.height(),
                                        )) as isize
                                    };
                                    let texture =
                                        Rc::new(Self::texture_from_image(display, image)?);
                                    entry.0 = metadata;
                                    entry.1 = texture;
                                    self.remaining_capacity += old_size_estimate;
                                    self.remaining_capacity -= size_estimate;
                                }
                            }
                            entry @ CachedTexture::LoadRequested => {
                                let texture = Rc::new(Self::texture_from_image(display, image)?);
                                *entry = CachedTexture::Texture((metadata, texture));
                                self.remaining_capacity -= size_estimate;
                            }
                        },
                    }
                }
                Err(TryRecvError::Disconnected) => return Ok(()),
                Err(TryRecvError::Empty) => break,
            }
        }

        Ok(())
    }

    pub fn send_load_requests(&mut self, dir_files: &Vec<fs::DirEntry>, current_index: usize) {
        use std::collections::btree_map::Entry;

        let mut index = current_index;

        let mut requested_images = 0;
        // Send as many load requests so that the estimated total will just fill the cache
        let mut estimated_remaining_cap = self.remaining_capacity;
        while estimated_remaining_cap > self.curr_est_size as isize {
            // Send a load request for the closest file not in the cache or outdated
            index += 1;
            if let Some(file) = dir_files.get(index) {
                let file_path = file.path();
                let file_name = if let Some(file_name) = file_path.file_name() {
                    file_name.to_owned()
                } else {
                    continue;
                };
                match self.texture_cache.entry(file_name) {
                    Entry::Vacant(entry) => {
                        if Self::is_file_supported(file_path.as_ref()) {
                            entry.insert(CachedTexture::LoadRequested);
                            self.path_tx.send(file.path()).unwrap();
                        }
                    }
                    Entry::Occupied(entry) => {
                        if let CachedTexture::Texture(ref entry) = entry.get() {
                            if entry.0.modified().unwrap()
                                != file.metadata().unwrap().modified().unwrap()
                            {
                                self.path_tx.send(file_path).unwrap();
                            }
                        }
                    }
                }
                estimated_remaining_cap -= self.curr_est_size as isize;
                requested_images += 1;
                if requested_images >= Self::MAX_BULK_PREFETCH_REQUEST {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn get(&self, name: &OsString) -> Option<&CachedTexture> {
        self.texture_cache.get(name)
    }

    pub fn load_specific(
        &mut self,
        display: &glium::Display,
        path: &PathBuf,
    ) -> Result<Rc<SrgbTexture2d>> {
        use std::collections::btree_map::Entry;

        let path = Path::new(path).canonicalize()?;
        let metadata = fs::metadata(path.as_path())?;

        let target_file_name = path.file_name().unwrap().to_owned();

        self.curr_dir = {
            let dir = path.parent().unwrap();
            if self.curr_dir != dir {
                self.texture_cache.clear();
                self.remaining_capacity = self.total_capacity;
            }
            dir.to_owned()
        };

        // Lets just process incoming images
        self.process_prefetched(display)?;

        // Check if it is inside the texture cache first
        {
            let texture_entry = self.texture_cache.entry(target_file_name.clone());
            if let Entry::Occupied(ref entry) = texture_entry {
                if let CachedTexture::Texture(ref entry) = entry.get() {
                    if entry.0.modified().unwrap() == metadata.modified().unwrap() {
                        return Ok(entry.1.clone());
                    }
                }
            }
        }

        let image = Self::load_image(path.as_path())?;
        self.curr_est_size =
            Self::get_image_size_estimate((image.width(), image.height())) as usize;
        let image_size_estimate = self.curr_est_size as isize;

        if self.remaining_capacity < image_size_estimate {
            // And there is just one more thing left to do...
            // Walk through our list of directory entries sorted by their distance from the current
            // file and in each step remove an entry from the cache until we reach the desired cache
            // size

            /*
            for (curr_name, texture) in self.texture_cache.iter() {
                if self.remaining_capacity < image_size_estimate && *curr_name != target_file_name {
                    let size_estimate = if let CachedTexture::Texture(ref entry) = texture {
                        Some(
                            Self::get_image_size_estimate((entry.1.width(), entry.1.height()))
                                as isize,
                        )
                    } else {
                        None
                    };
                    if let Some(size_estimate) = size_estimate {
                        entry.remove();
                        self.remaining_capacity += size_estimate;
                    }
                } else {
                    break;
                }
            }
            */
            let mut passed_current = false;
            let mut remaining_capacity = self.remaining_capacity;
            let mut tmp_cache = BTreeMap::new();
            mem::swap(&mut self.texture_cache, &mut tmp_cache);
            tmp_cache
                .into_iter()
                .filter(|(curr_name, texture)| {
                    if **curr_name == target_file_name {
                        passed_current = true;
                    }
                    if !passed_current && remaining_capacity < image_size_estimate {
                        if let CachedTexture::Texture(ref entry) = texture {
                            remaining_capacity -=
                                Self::get_image_size_estimate((entry.1.width(), entry.1.height()))
                                    as isize;
                            false
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                })
                .fold((), |_, (curr_name, texture)| {
                    self.texture_cache.insert(curr_name.clone(), texture);
                });
            self.remaining_capacity = remaining_capacity;
        }
        self.remaining_capacity -= image_size_estimate;

        let result_texture = Rc::new(Self::texture_from_image(display, image)?);
        match self.texture_cache.entry(target_file_name.clone()) {
            Entry::Vacant(entry) => {
                entry.insert(CachedTexture::Texture((metadata, result_texture.clone())));
            }
            Entry::Occupied(mut entry) => match entry.get_mut() {
                entry @ CachedTexture::LoadRequested => {
                    *entry = CachedTexture::Texture((metadata, result_texture.clone()));
                }
                CachedTexture::Texture(ref mut entry) => {
                    if entry.0.modified().unwrap() != metadata.modified().unwrap() {
                        *entry = (metadata, result_texture.clone());
                    }
                },
            },
        }

        //self.send_load_requests();

        Ok(result_texture)
    }


    pub fn load_image(image_path: &Path) -> Result<image::RgbaImage> {
        Ok(image::open(image_path)?.to_rgba())
    }

    fn texture_from_image(
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

        Ok(SrgbTexture2d::with_mipmaps(
            display,
            image,
            mipmaps,
        )?)
    }

    fn get_image_size_estimate(dimensions: (u32, u32)) -> u32 {
        // counting all the mipmaps would add an additionnal multiplier of around ~1.6
        // but only the gpu textures have mip maps so just multiply by 1.5
        ((dimensions.0 * dimensions.1 * 4) as f32 * 1.5) as u32
    }

    pub fn is_file_supported(filename: &Path) -> bool {
        if let Some(ext) = filename.extension() {
            if let Some(ext) = ext.to_str() {
                let ext = ext.to_lowercase();
                match ext.as_str() {
                    "png" | "jpg" | "bmp" | "gif" | "tiff" | "webp" | "pnm" => {
                        return true;
                    }
                    _ => (),
                }
            }
        }

        false
    }
}

impl Drop for TextureLoader {
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
