
use std::path::Path;

use glium;
use glium::texture::{RawImage2d, SrgbTexture2d};

use image;

pub fn load_texture_without_cache(display: &glium::Display, image_path: &Path) -> SrgbTexture2d {
    let image = image::open(image_path).unwrap().to_rgba();

    texture_from_image(display, image)
}

pub fn texture_from_image(display: &glium::Display, image: image::RgbaImage) -> SrgbTexture2d {
    let image_dimensions = image.dimensions();
    let image = RawImage2d::from_raw_rgba(image.into_raw(), image_dimensions);

    SrgbTexture2d::with_mipmaps(display, image, glium::texture::MipmapsOption::NoMipmap).unwrap()
}
