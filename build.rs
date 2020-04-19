#[cfg(windows)]
extern crate winres;

use std::env;
use std::fs;
use std::path::Path;

#[cfg(windows)]
fn platform_specific() {
	let mut res = winres::WindowsResource::new();
	res.set_icon("resource_dev/emulsion.ico");
	res.compile().unwrap();
}

#[cfg(unix)]
fn platform_specific() {}

fn main() {
	platform_specific();

	let dir_name = "resource";
	let profile = env::var("PROFILE").unwrap();

	let target_resource_path = Path::new("target").join(profile).join(dir_name);
	fs::create_dir_all(target_resource_path.clone()).unwrap();

	for entry in fs::read_dir("resource/").unwrap() {
		let entry = entry.unwrap();
		if entry.file_type().unwrap().is_file() {
			fs::copy(entry.path(), target_resource_path.join(entry.file_name())).unwrap();
		}
	}
}
