use std::rc::{Rc, Weak};

use std::time::{Duration, Instant};

use gelatin::{label::Label, misc::*, picture::Picture, NextUpdate, Widget};

static COPY_STARTED: &[u8] = include_bytes!("../../resource/copy-started.png");
static COPY_READY: &[u8] = include_bytes!("../../resource/copy-ready.png");
static COPY_FAILED: &[u8] = include_bytes!("../../resource/copy-failed.png");

const READY_DISPLAY_TIME: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct CopyNotifications {
	pub widget: Weak<Label>,
	copy_started_img: Rc<Picture>,
	copy_ready_img: Rc<Picture>,
	copy_failed_img: Rc<Picture>,
	finished: bool,
	finished_time: std::time::Instant,
}

impl CopyNotifications {
	pub fn new(widget: &Rc<Label>) -> CopyNotifications {
		let copy_started_img = Rc::new(Picture::from_encoded_bytes(COPY_STARTED));
		let copy_ready_img = Rc::new(Picture::from_encoded_bytes(COPY_READY));
		let copy_failed_img = Rc::new(Picture::from_encoded_bytes(COPY_FAILED));

		widget.set_icon(None);
		widget.set_ignore_layout(true);
		widget.set_width(Length::Fixed(128.0));
		widget.set_height(Length::Fixed(32.0));
		widget.set_margin_all(4.0);
		widget.set_horizontal_align(Alignment::End);
		widget.set_vertical_align(Alignment::End);
		widget.set_shadow_size(0.15);
		widget.set_visible(false);

		CopyNotifications {
			widget: Rc::downgrade(widget),
			copy_started_img,
			copy_ready_img,
			copy_failed_img,
			finished: true,
			finished_time: Instant::now(),
		}
	}

	pub fn set_started(&mut self) {
		let widget = self.widget.upgrade().unwrap();
		widget.set_icon(Some(self.copy_started_img.clone()));
		widget.set_visible(true);
		self.finished = false;
	}

	pub fn set_finished(&mut self, succeeded: bool) {
		let widget = self.widget.upgrade().unwrap();
		let icon = if succeeded {
			self.copy_ready_img.clone()
		} else {
			self.copy_failed_img.clone()
		};
		widget.set_icon(Some(icon));
		self.finished_time = Instant::now();
		self.finished = true;
	}

	pub fn update(&mut self) -> NextUpdate {
		let widget = self.widget.upgrade().unwrap();
		if widget.visible() && self.finished {
			let now = Instant::now();
			if now.duration_since(self.finished_time) > READY_DISPLAY_TIME {
				widget.set_visible(false);
				NextUpdate::Latest
			} else {
				NextUpdate::WaitUntil(self.finished_time + READY_DISPLAY_TIME)
			}
		} else {
			NextUpdate::Latest
		}
	}
}
