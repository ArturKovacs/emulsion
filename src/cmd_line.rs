use crate::Version;
use clap::{App, Arg};
use std::path::PathBuf;

/// Parses the command-line arguments and returns the file path
pub fn parse_args(config_path: &PathBuf, cache_path: &PathBuf) -> Option<String> {
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
		.arg(Arg::with_name("PATH").help("The file path of the image").index(1))
		.get_matches();

	matches.value_of("PATH").map(ToString::to_string)
}
