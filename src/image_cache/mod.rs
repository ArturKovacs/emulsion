use std::ffi::{OsStr, OsString};
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

pub use self::errors::Result;
use self::errors::*;

pub struct ImageCache {
    dir_path: PathBuf,
    //current_name: OsString,
    current_index: usize,
    dir_files: Vec<fs::DirEntry>,

    loader: TextureLoader,
}

/// This is a store for the supported images loaded from a folder
///
/// The basic idea is to have a few images already in the memory while an image is shown on the screen
impl ImageCache {
    /// # Arguments
    /// * `capacity` - Number of bytes. The last image loaded will be the one at which the allocated memory reaches or exceeds capacity
    pub fn new(capacity: isize, threads: u32) -> ImageCache {
        ImageCache {
            dir_path: PathBuf::new(),
            current_index: 0,
            dir_files: Vec::new(),

            loader: TextureLoader::new(capacity, threads),
        }
    }

    pub fn current_filename(&self) -> OsString {
        match self.dir_files.get(self.current_index) {
            Some(entry) => entry.file_name(),
            None => OsString::new(),
        }
    }

    pub fn current_file_path(&self) -> PathBuf {
        self.dir_path.join(self.current_filename()).to_owned()
    }

    pub fn current_file_index(&self) -> usize {
        self.current_index
    }

    pub fn current_dir_len(&self) -> usize {
        self.dir_files.len()
    }

    pub fn update_directory(&mut self) -> Result<()> {
        let curr_filename = self.current_filename();
        self.dir_files = Self::collect_directory(self.dir_path.as_path())?;

        for (index, entry) in self.dir_files.iter().enumerate() {
            if entry.file_name() == curr_filename {
                self.current_index = index;
                return Ok(());
            }
        }

        Err(format!(
            "Could not find file '{}' in directory '{}'",
            curr_filename.to_str().unwrap(),
            self.dir_path.to_str().unwrap()
        ).into())
    }

    pub fn load_at_index(
        &mut self,
        display: &glium::Display,
        index: usize,
    ) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        let path = self.dir_files
            .get(index)
            .ok_or_else(|| {
                format!(
                    "Index {} is out of bounds of the current directory '{}'",
                    index,
                    self.dir_path.to_str().unwrap()
                )
            })?
            .path();

        let result = self.loader.load_specific(display, &path)?;

        self.current_index = index;

        Ok((
            result,
            path.file_name().unwrap_or(OsStr::new("")).to_owned(),
        ))
    }

    pub fn load_specific(
        &mut self,
        display: &glium::Display,
        path: &Path,
    ) -> Result<Rc<SrgbTexture2d>> {
        let path = path.canonicalize()?;

        let result = self.loader.load_specific(display, &path)?;

        let current_name = match path.file_name() {
            Some(filename) => filename.to_owned(),
            None => bail!(format!(
                "Could not get filename from path '{}'",
                path.to_str().unwrap()
            )),
        };

        // Directory may have changed
        let parent = path.parent()
            .ok_or("Could not get parent directory")?
            .to_owned();
        if self.dir_path != parent {
            self.change_directory(parent, current_name)?;
        } else {
            for (index, entry) in self.dir_files.iter().enumerate() {
                if entry.file_name() == current_name {
                    self.current_index = index;
                }
            }
        }

        Ok(result)
    }

    pub fn load_next(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        self.load_jump(display, 1)
    }

    pub fn load_prev(&mut self, display: &glium::Display) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        self.load_jump(display, -1)
    }

    pub fn load_jump(
        &mut self,
        display: &glium::Display,
        jump_count: i32,
    ) -> Result<(Rc<SrgbTexture2d>, OsString)> {
        if jump_count == 0 {
            let filename = self.current_filename();
            return Ok((
                match self.loader.get(&filename) {
                    Some(CachedTexture::Texture(ref entry)) => entry.1.clone(),
                    _ => bail!(Error::from("Could not find current file in cache.")),
                },
                filename,
            ));
        }

        if self.dir_files.len() == 0 {
            return Err("Folder is empty or no folder was open when trying to load image.".into());
        }

        let mut target_index =
            (self.current_index as isize + jump_count as isize) % self.dir_files.len() as isize;
        if target_index < 0 {
            target_index += self.dir_files.len() as isize;
        }

        let target_path = self.dir_files.get(target_index as usize).unwrap().path();
        let result = self.loader.load_specific(display, &target_path)?;
        self.current_index = target_index as usize;

        Ok((
            result,
            target_path.file_name().unwrap_or(OsStr::new("")).to_owned(),
        ))
    }

    pub fn process_prefetched(&mut self, display: &glium::Display) -> Result<()> {
        Ok(self.loader.process_prefetched(display)?)
    }

    pub fn send_load_requests(&mut self) {
        self.loader
            .send_load_requests(&self.dir_files, self.current_index);
    }

    fn change_directory(&mut self, dir_path: PathBuf, filename: OsString) -> Result<()> {
        self.dir_files = Self::collect_directory(dir_path.as_path())?;

        // Look up the index of the filename in the directory
        for (index, entry) in self.dir_files.iter().enumerate() {
            if entry.file_name() == filename {
                self.current_index = index;
                self.dir_path = dir_path;
                return Ok(());
            }
        }

        Err(format!(
            "Could not find file '{}' in directory '{}'",
            filename.to_str().unwrap(),
            dir_path.to_str().unwrap()
        ).into())
    }

    fn collect_directory(path: &Path) -> Result<Vec<fs::DirEntry>> {
        let mut dir_files: Vec<_> = fs::read_dir(path)?
            .filter_map(|x| match x.ok() {
                Some(entry) => match entry.file_type().ok() {
                    Some(file_type) => if file_type.is_file() {
                        if TextureLoader::is_file_supported(entry.path().as_path()) {
                            Some(entry)
                        } else {
                            None
                        }
                    } else {
                        None
                    },
                    None => None,
                },
                None => None,
            })
            .collect();

        dir_files.sort_unstable_by(|a, b| a.file_name().cmp(&b.file_name()));

        Ok(dir_files)
    }
}
