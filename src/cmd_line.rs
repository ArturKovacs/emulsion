use crate::Version;
use clap::{App, Arg};
use std::path::PathBuf;

pub struct Args {
	pub file_path: Option<String>,
	pub displayed_folders: Option<u32>,
}

/// Parses the command-line arguments and returns the file path
pub fn parse_args(config_path: &PathBuf, cache_path: &PathBuf) -> Args {
	let config = format!(
		"CONFIGURATION:\n    config file: {}\n    cache file:  {}",
		config_path.to_string_lossy(),
		cache_path.to_string_lossy(),
	);

	let matches = App::new("emulsion")
		.version(Version::cargo_pkg_version().to_string().as_str())
		.author("Artur Barnabas <kovacs.artur.barnabas@gmail.com>")
		.about(
			"A fast and minimalistic image viewer\n\
			https://arturkovacs.github.io/emulsion-website/",
		)
		.after_help(config.as_str())
		.arg(
			Arg::with_name("FOLDERS")
				.long("folders")
				.short("f")
				.help("Number of folders to display")
				.takes_value(true)
				.validator(|v| match v.parse::<u32>() {
					Ok(_) => Ok(()),
					Err(e) => Err(format!("{}: '{}'", e, v)),
				}),
		)
		.arg(
			Arg::with_name("absolute")
				.long("absolute")
				.short("a")
				.help("Show absolute file path")
				.takes_value(false)
				.conflicts_with("FOLDERS"),
		)
		.arg(Arg::with_name("PATH").help("The file path of the image").index(1))
		.get_matches();

	let file_path = matches.value_of("PATH").map(ToString::to_string);

	let displayed_folders = if matches.is_present("absolute") {
		Some(std::u32::MAX)
	} else {
		matches.value_of("FOLDERS").map(|s| s.parse::<u32>().unwrap())
	};

	Args { file_path, displayed_folders }
}
