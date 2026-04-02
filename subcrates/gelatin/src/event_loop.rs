use std::{fmt::Debug, rc::Rc};

use crate::{
	application::Application,
	window::{Window, WindowDescriptor},
};

use winit::event_loop::EventLoop as WinitEventLoop;

pub struct EventLoop<UserEvent: Debug + 'static> {
	pub(crate) inner: winit::event_loop::EventLoop<UserEvent>,
}

impl<UserEvent> EventLoop<UserEvent>
where
	UserEvent: Debug + 'static,
{
	pub fn new() -> Self {
		Self { inner: WinitEventLoop::<UserEvent>::with_user_event().build().unwrap() }
	}

	pub fn create_proxy(&self) -> winit::event_loop::EventLoopProxy<UserEvent> {
		self.inner.create_proxy()
	}
}

impl<UserEvent> Default for EventLoop<UserEvent>
where
	UserEvent: Debug + 'static,
{
	fn default() -> Self {
		Self::new()
	}
}

pub struct ActiveEventLoop<'a> {
	pub(crate) inner: &'a winit::event_loop::ActiveEventLoop,
	pub(crate) application: &'a mut Application,
}

impl<'a> ActiveEventLoop<'a> {
	pub fn create_window(
		&mut self,
		desc: WindowDescriptor,
	) -> Result<Rc<Window>, Box<dyn std::error::Error>> {
		Window::new(self.application, desc, self.inner)
	}
}
