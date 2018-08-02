
use std::rc::Rc;
use std::boxed::Box;

use glium;
use glium::{Surface, Frame};
use glium::texture::SrgbTexture2d;
use glium::glutin;

use cgmath::{Matrix4, Vector2};

use ui::{ElementFunctions, DrawContext, Event};


pub struct Label {
    texture: Rc<SrgbTexture2d>,
    position: Vector2<f32>,
}
