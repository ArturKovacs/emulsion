use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;

use super::image_loader::*;

/// This struct is used in a map to determine the appropriate file when
/// a load result comes in. (Load results identify the file with the request id only)
/// See: `ImageCache::ongoing_requests`
pub struct PendingRequestInfo {
	//path: PathBuf,
	cancelled: bool,
	finished: bool,
	// mod_time: Option<SystemTime>,

	// I know that it would probably be faster to use a VecDeque and `drain` the elements
	// instead of moving the entire vector in the `take_results` function.
	// But after multiple attempts and a long and tiring fight with the borrow checker
	// I gave up.
	results: Vec<LoadResult>,
}

impl PendingRequestInfo {
	pub fn cancel(&mut self) {
		self.cancelled = true;
	}
}

pub struct PendingRequests {
	/// Keeps track of all requests that have been sent
	/// but for which no response has been received
	by_id: HashMap<u32, PendingRequestInfo>,
	path_to_id: HashMap<PathBuf, u32>,
}

impl PendingRequests {
	pub fn new() -> Self {
		PendingRequests { by_id: HashMap::new(), path_to_id: HashMap::new() }
	}

	pub fn take_results(&mut self, id: u32) -> Option<Vec<LoadResult>> {
		if let Entry::Occupied(mut entry) = self.by_id.entry(id) {
			if entry.get().finished {
				let info = entry.remove_entry().1;
				Some(info.results)
			} else {
				let mut result = Vec::with_capacity(3);
				std::mem::swap(&mut result, &mut (entry.get_mut().results));
				Some(result)
			}
		} else {
			None
		}
	}

	/// This returns all the ids including the finished item's
	pub fn get_all_ids(&self) -> Vec<u32> {
		self.by_id.keys().copied().collect()
	}

	pub fn cancelled(&self, id: &u32) -> Option<bool> {
		self.get(id).map(|i| i.cancelled)
	}

	pub fn contains(&self, id: &u32) -> bool {
		if let Some(info) = self.by_id.get(id) {
			!info.finished
		} else {
			false
		}
	}

	pub fn set_finished(&mut self, id: &u32) {
		if let Entry::Occupied(mut entry) = self.by_id.entry(*id) {
			if entry.get().results.is_empty() {
				entry.remove_entry();
			} else {
				entry.get_mut().finished = true;
			}
		}
	}

	pub fn len(&self) -> usize {
		self.iter().count()
	}

	/// Returns true if the result was appended to a pending request.
	/// Returns false otherwise.
	pub fn add_load_result(&mut self, load_result: LoadResult) {
		if let Some(info) = self.by_id.get_mut(&load_result.req_id()) {
			info.results.push(load_result);
		} else {
			unreachable!()
		}
	}

	pub fn add_request(&mut self, request: LoadRequest) {
		self.path_to_id.insert(request.path.clone(), request.req_id);
		self.by_id.insert(
			request.req_id,
			PendingRequestInfo {
				cancelled: false,
				//path: request.path,
				finished: false,
				results: Vec::with_capacity(3),
			},
		);
	}

	pub fn get(&self, id: &u32) -> Option<&PendingRequestInfo> {
		self.by_id.get(id).filter(|i| !i.finished)
	}

	pub fn iter_mut(&mut self) -> impl Iterator<Item = (&u32, &mut PendingRequestInfo)> {
		self.by_id.iter_mut().filter(|(_, i)| !i.finished)
	}

	pub fn iter(&self) -> impl Iterator<Item = (&u32, &PendingRequestInfo)> {
		self.by_id.iter().filter(|(_, i)| !i.finished)
	}
}
