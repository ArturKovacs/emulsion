use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use glium;

use glium::texture::SrgbTexture2d;

mod texture_loader;
use self::texture_loader::{CachedTexture, TextureLoader};

pub mod errors {
    use glium::texture;
    use image;
    use image_cache::texture_loader;
    use std::io;

    error_chain!{
        foreign_links {
            Io(io::Error) #[doc = "Error during IO"];
            TextureCreationError(texture::TextureCreationError);
            ImageLoadError(image::ImageError);
            TextureLoaderError(texture_loader::errors::Error);
        }
    }
}

use self::errors::*;
pub use self::errors::Result;

pub struct ImageCache {
    dir_path: PathBuf,
    current_name: OsString,
    dir_files: Vec<fs::DirEntry>,

    loader: TextureLoader,
}

/// This is a store for the supported images loaded from a folder
/// 
/// The basic idea is to have a few images already in the memory while an image is shown on the screen
impl ImageCache {
    /// # Arguemnts
    /// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
    pub fn new(capacity: isize, threads: u32) -> ImageCache {
        ImageCache {
            dir_path: PathBuf::new(),
            current_name: OsString::new(),
            dir_files: Vec::new(),

            loader: TextureLoader::new(capacity, threads),
        }
    }

    pub fn current_filename<'a>(&'a self) -> &'a OsString {
        &self.current_name
    }

    pub fn current_file_path(&self) -> PathBuf {
        self.dir_path.join(self.current_name.clone()).to_owned()
    }

    pub fn update_directory(&mut self) -> Result<()> {
        self.dir_files = fs::read_dir(self.dir_path.as_path())?
            .filter_map(|x| {
                let entry = x.unwrap();
                if entry.file_type().unwrap().is_file() {
                    Some(entry)
                } else {
                    None
                }
            })
            .collect();

        self.dir_files
            .sort_unstable_by(|a, b| a.file_name().cmp(&b.file_name()));

        Ok(())
    }

    pub fn load_specific(
        &mut self,
        display: &glium::Display,
        path: &Path,
    ) -> Result<Rc<SrgbTexture2d>> {
        let path = path.canonicalize()?;

        self.current_name = match path.file_name() {
            Some(filename) => filename.to_owned(),
            None => bail!(format!(
                "Could not get filename for path '{}'",
                path.to_str().unwrap()
            )),
        };

        // Directory may have changed
        let parent = path.parent().unwrap().to_owned(); // It absolutely must have a parent if it was a file
        if self.dir_path != parent {
            self.dir_path = parent;
            self.update_directory()?;
        }

        return Ok(self.loader.load_specific(display, &path)?);
    }

    pub fn load_next(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        let iter = self.dir_files.iter().chain(self.dir_files.iter());
        let result = self.loader.load_iter_next(display, iter, &self.current_name);
        match result {
            Ok((_, ref filename)) => {
                self.current_name = filename.clone();
            }
            _ => (),
        }

        Ok(result?)
    }

    pub fn load_prev(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        let iter = self.dir_files.iter().chain(self.dir_files.iter()).rev();
        let result = self.loader.load_iter_next(display, iter, &self.current_name);
        match result {
            Ok((_, ref filename)) => {
                self.current_name = filename.clone();
            }
            _ => (),
        }

        Ok(result?)
    }

    pub fn load_jump(
        &mut self,
        display: &glium::Display,
        jump_count: i32,
    ) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        if jump_count == 0 {
            return Ok((
                match self.loader.get(&self.current_name) {
                    Some(CachedTexture::Texture(ref entry)) => entry.1.clone(),
                    _ => bail!(Error::from("Could not find current file in cache.")),
                },
                self.current_name.clone(),
            ));
        }

        let forward_iter = self.dir_files.iter().chain(self.dir_files.iter());
        let result = if jump_count < 0 {
            self.loader.load_iter_jump(
                display,
                forward_iter.rev(),
                -jump_count as u32,
                &self.current_name,
            )
        } else {
            self.loader
                .load_iter_jump(display, forward_iter, jump_count as u32, &self.current_name)
        };

        match result {
            Ok((_, ref filename)) => {
                self.current_name = filename.clone();
            }
            _ => (),
        }

        Ok(result?)
    }

    pub fn process_prefetched(&mut self, display: &glium::Display) -> Result<()> {
        Ok(self.loader.process_prefetched(display)?)
    }

    pub fn send_load_requests(&mut self) {
        self.loader
            .send_load_requests(&self.dir_files, &self.current_name);
    }
}
