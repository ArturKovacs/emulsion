#[cfg(windows)]
extern crate winres;

use std::env;
use std::fs;
use std::path::Path;

#[cfg(windows)]
fn platform_specific() -> std::io::Result<()> {
	let mut res = winres::WindowsResource::new();
	res.set_icon("resource_dev/emulsion.ico");
	res.compile()?;
	Ok(())
}

#[cfg(unix)]
fn platform_specific() -> std::io::Result<()> {
	Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
	platform_specific()?;

	let dir_name = "resource";
	let profile = env::var("PROFILE")?;

	let target_resource_path = Path::new("target").join(profile).join(dir_name);
	fs::create_dir_all(target_resource_path.clone())?;

	for entry in fs::read_dir("resource/")? {
		let entry = entry?;
		if entry.file_type()?.is_file() {
			fs::copy(entry.path(), target_resource_path.join(entry.file_name()))?;
		}
	}
	Ok(())
}
