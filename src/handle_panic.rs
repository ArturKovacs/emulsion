use std::env;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::iter;
use std::panic;
use std::string::String;

use backtrace::Backtrace;

use crate::PROJECT_DIRS;

pub fn handle_panic(info: &panic::PanicInfo) {
	let trace = Backtrace::new();

	let mut msg = String::new();

	let payload = info.payload();
	let payload_string = payload
		.downcast_ref::<&str>()
		.copied()
		.or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()));

	msg.push('\n');
	if let Some(panic_message) = payload_string {
		msg.push_str(&format!("\n--\n{}\n--\n\n", panic_message));
	}
	if let Some(location) = info.location() {
		msg.push_str(&format!(
			"Location {}:{}:{}\n\n",
			location.file(),
			location.line(),
			location.column()
		));
	}
	msg.push_str(&format!("{:?}\n", trace));
	for ch in iter::repeat('=').take(99) {
		msg.push(ch);
	}

	eprintln!("\nPanic happened{}", &msg);
	write_to_file(&msg).expect("Could not write panic to file.");
}

fn write_to_file(msg: &str) -> io::Result<()> {
	let local_data_folder;
	if let Some(ref project_dirs) = *PROJECT_DIRS {
		local_data_folder = project_dirs.data_local_dir().to_owned();
	} else {
		let curr_exe = env::current_exe()?;
		let curr_exe_dir = curr_exe.parent().ok_or_else(|| {
			io::Error::new(io::ErrorKind::Other, "Could not get exe parent folder!")
		})?;
		local_data_folder = curr_exe_dir.to_owned();
	}
	if !local_data_folder.exists() {
		std::fs::create_dir_all(&local_data_folder).unwrap();
	}
	let mut file =
		OpenOptions::new().create(true).append(true).open(local_data_folder.join("panic.txt"))?;

	write!(file, "{}", msg)?;
	Ok(())
}
