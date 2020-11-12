use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use super::image_loader::is_file_supported;
use crate::parallel_action::ParallelAction;

#[derive(Debug)]
pub enum Error {
	WaitingOnFolderFilter,
	Other(String),
}
impl fmt::Display for Error {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Error::WaitingOnFolderFilter => {
				f.write_str("The directory is still being filtered for images")
			}
			Error::Other(s) => f.write_fmt(format_args!("Other error: {}", s)),
		}
	}
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
	fn from(e: std::io::Error) -> Self {
		Error::Other(format!("{}", e))
	}
}

pub type Result<T> = std::result::Result<T, Error>;

macro_rules! step_to_next_img {
	($this:ident, $iter:ident) => {
		for (i, file) in $iter {
			if is_file_supported(&file.path) {
				$this.curr_file_idx = i;
				$this.set_image_index_from_file_index();
				return;
				}
			}
	};
}

#[derive(Clone)]
pub struct DirItem {
	pub path: PathBuf,

	/// Sometimes also abbreviated as `req_id` is used as a more efficient replacement
	/// of a PathBuf to identify a file load request.
	pub request_id: u32,
}

// enum FilterState {
//     Idle,
//     Processing(Arc<Vec<usize>>),
//     JustFinished(Arc<Vec<usize>>),
// }

pub struct Directory {
	path: PathBuf,
	files: Vec<DirItem>,

	/// Maps image indicies to indicies for the `files` vector.
	/// For example one could use it like `files[image_indicies[i]]`
	img_i_to_file_i: Vec<usize>,

	/// Maps file indicies to indicies for the `curr_image_idx`.
	/// This is relevant when the current image is given by its name
	/// when it will first be located by its file index.
	file_i_to_img_i: Vec<Option<u32>>,

	/// A monotonically increasing integer used for identifying
	/// each load request
	current_req_id: u32,

	/// current file index
	/// This must never be exposed to users of this object.
	curr_file_idx: usize,

	/// Current image index.
	/// Use this value to index the `image_indicies` vector to find the apppropriate file index.
	curr_image_idx: usize,

	//filter_state: Arc<Mutex<FilterState>>,
	filter_action: ParallelAction<Vec<DirItem>, Vec<usize>>,
}

fn get_action() -> impl FnMut(Vec<DirItem>) -> Vec<usize> {
	|input: Vec<DirItem>| {
		input
			.into_iter()
			.enumerate()
			.filter_map(|(i, item)| if is_file_supported(&item.path) { Some(i) } else { None })
			.collect()
	}
}

impl Directory {
	pub fn new() -> Self {
		Directory {
			path: PathBuf::new(),
			files: Vec::new(),
			img_i_to_file_i: Vec::new(),
			file_i_to_img_i: Vec::new(),
			curr_file_idx: 0,
			curr_image_idx: 0,
			current_req_id: 0,
			filter_action: ParallelAction::new(get_action()),
		}
	}

	pub fn change_directory(&mut self, path: &Path) -> Result<()> {
		if self.path != path {
			self.path = path.to_owned();
			self.collect_directory()
		} else {
			Ok(())
		}
	}

	pub fn change_directory_with_filename(&mut self, path: &Path, filename: &OsStr) -> Result<()> {
		self.change_directory(path)?;
		// Look up the index of the filename in the directory
		for (index, desc) in self.files.iter().enumerate() {
			if desc.path.file_name().unwrap() == filename {
				self.curr_file_idx = index;
				self.set_image_index_from_file_index();
				// If we already finished filtering somehow
				self.check_filter_ready();
				return Ok(());
			}
		}

		Err(Error::Other(format!("Could not find file {:?} in directory {:?}", filename, path)))
	}

	pub fn curr_filename(&self) -> OsString {
		match self.files.get(self.curr_file_idx) {
			Some(n) => n.path.file_name().unwrap().to_owned(),
			None => OsString::new(),
		}
	}

	pub fn curr_descriptor(&self) -> Option<&DirItem> {
		self.files.get(self.curr_file_idx)
	}

	pub fn path(&self) -> &Path {
		self.path.as_path()
	}

	pub fn set_curr_img_index(&mut self, index: usize) -> Result<()> {
		if !self.check_filter_ready() {
			return Err(Error::WaitingOnFolderFilter);
		}
		if let Some(file_idx) = self.img_i_to_file_i.get(index) {
			if *file_idx < self.files.len() {
				self.curr_file_idx = *file_idx;
				self.curr_image_idx = index;
				return Ok(());
			}
		}
		Err(Error::Other(format!("Could not find image index")))
	}

	pub fn jump_to_prev(&mut self) {
		let skip = (self.files.len() - 1) - self.curr_file_idx;
		let iter =
			self.files.iter().enumerate().rev().cycle().skip(skip).take(self.files.len()).skip(1);
		step_to_next_img!(self, iter);
	}

	pub fn jump_to_next(&mut self) {
		// Go forwards until a valid image is found or until we arrived back to the starting file
		let iter = self
			.files
			.iter()
			.enumerate()
			.cycle()
			.skip(self.curr_file_idx)
			.take(self.files.len())
			.skip(1);
		step_to_next_img!(self, iter);
	}

	/// Returns none when the folder hasn't finished filtering
	pub fn curr_img_index(&mut self) -> Option<usize> {
		if !self.check_filter_ready() {
			return None;
		}
		Some(self.curr_image_idx)
	}

	/// If the image count for the current folder is already known, returns Some(n)
	/// where n is the number of images in the folder.
	/// If the image count is not yet know, it returns None.
	pub fn image_count(&mut self) -> Option<usize> {
		if !self.check_filter_ready() {
			return None;
		}
		Some(self.img_i_to_file_i.len())
	}

	/// Return None if the number of images haven't been calculated yet
	pub fn image_by_index(&mut self, idx: usize) -> Option<&DirItem> {
		if !self.check_filter_ready() {
			return None;
		}
		if let Some(i) = self.img_i_to_file_i.get(idx) {
			Some(&self.files[*i])
		} else {
			None
		}
	}

	pub fn update_directory(&mut self) -> Result<()> {
		let (curr_filename, curr_index) = (self.curr_filename(), self.curr_file_idx);
		self.collect_directory()?;
		// if possible preserve previous file_name
		for (index, desc) in self.files.iter().enumerate() {
			if desc.path.file_name().unwrap() == curr_filename {
				self.curr_file_idx = index;
				self.set_image_index_from_file_index();
				self.check_filter_ready();
				return Ok(());
			}
		}
		// if is_file_supported, preserve index of previous file or its following files
		for (index, desc) in self.files.iter().enumerate().skip(curr_index) {
			if is_file_supported(&PathBuf::from(desc.path.file_name().unwrap())) {
				self.curr_file_idx = index;
				self.set_image_index_from_file_index();
				self.check_filter_ready();
				return Ok(());
			}
		}
		if self.files.len() <= self.curr_file_idx && !self.files.is_empty() {
			self.curr_file_idx = 0;
		}

		Ok(())
	}

	pub fn collect_directory(&mut self) -> Result<()> {
		let mut dir_files: Vec<_> = fs::read_dir(&self.path)?
			.filter_map(|x| match x {
				Ok(entry) => match entry.file_type() {
					Ok(file_type) => {
						if file_type.is_file() || file_type.is_symlink() {
							self.current_req_id += 1;
							Some(DirItem { path: entry.path(), request_id: self.current_req_id })
						} else {
							None
						}
					}
					Err(_) => None,
				},
				Err(_) => None,
			})
			.collect();

		dir_files.sort_unstable_by(|a, b| {
			lexical_sort::natural_lexical_cmp(
				&a.path.file_name().unwrap().to_string_lossy(),
				&b.path.file_name().unwrap().to_string_lossy(),
			)
		});

		// Set the current file index to the first image
		for (i, item) in dir_files.iter().enumerate() {
			if is_file_supported(&item.path) {
				self.curr_file_idx = i;
				break;
			}
		}
		self.filter_action.give_input(dir_files.clone());
		self.img_i_to_file_i.clear();
		self.file_i_to_img_i.clear();
		self.files = dir_files;
		Ok(())
	}

	fn finished_filtering(&mut self) {
		self.file_i_to_img_i.clear();
		self.file_i_to_img_i.reserve(self.files.len());
		let mut last_file_i: isize = -1;
		for (curr_img_i, &curr_file_i) in self.img_i_to_file_i.iter().enumerate() {
			for _ in (last_file_i + 1) as usize..curr_file_i {
				self.file_i_to_img_i.push(None);
			}
			self.file_i_to_img_i.push(Some(curr_img_i as u32));
			last_file_i = curr_file_i as isize;
		}
		self.set_image_index_from_file_index();
	}

	fn set_image_index_from_file_index(&mut self) {
		if let Some(img_idx) = self.file_i_to_img_i.get(self.curr_file_idx) {
			self.curr_image_idx = img_idx.unwrap() as usize;
		}
	}

	fn check_filter_ready(&mut self) -> bool {
		if let Some(out) = self.filter_action.try_get_output() {
			self.img_i_to_file_i = out;
			self.finished_filtering();
			return true;
		}
		self.filter_action.is_ready()
	}
}
