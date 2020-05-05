use crate::Version;

static USAGE: &str = "USAGE:
    emulsion [OPTIONS] <FILE PATH>";

static OPTIONS: &str = "OPTIONS:
    -h, --help      Prints help information
    -V, --version   Prints version information";

fn print_usage() {
	println!("{}", USAGE);
	println!("\nFor more information try --help");
}

fn print_help() {
	print_version();
	println!("A fast and minimalistic image viewer");
	println!("Artur Barnabas <kovacs.artur.barnabas@gmail.com>");
	println!("\n{}", USAGE);
	println!("\n{}", OPTIONS);
}

fn print_version() {
	println!("emulsion {}", Version::cargo_pkg_version());
}

/// Contains the command-line arguments
struct Args {
	help: bool,
	version: bool,
	file_path: Option<String>,
}

/// Parses the command-line arguments
fn parse_args() -> Result<Args, String> {
	let mut help = false;
	let mut version = false;
	let mut file_path = None;

	for arg in std::env::args().skip(1) {
		if arg.starts_with("--") {
			// parse argument
			match arg.as_str() {
				"--help" => help = true,
				"--version" => version = true,
				_ => return Err(format!("Argument '{}' supported", arg)),
			}
		} else if arg == "-help" {
			help = true;
		} else if arg.starts_with("-") && arg.len() > 1 {
			// parse flags
			for flag in arg[1..].chars() {
				match flag {
					'h' => help = true,
					'V' => version = true,
					_ => return Err(format!("Flag '-{}' not supported", flag)),
				}
			}
		} else {
			if file_path.is_some() {
				return Err(format!("More than one file argument provided"));
			}
			file_path = Some(arg);
		}
	}

	Ok(Args { help, version, file_path })
}

/// Return the file path passed to emulsion.
///
/// If the help or version flag was used, display the requested information instead.
pub fn get_file_path() -> Option<String> {
	match parse_args() {
		Ok(args) => {
			if args.help {
				print_help();
				return None;
			} else if args.version {
				print_version();
				return None;
			} else if args.file_path.is_none() {
				eprintln!("Error: No image file provided\n");
				print_usage();
				return None;
			}
			return args.file_path;
		}
		Err(msg) => {
			eprintln!("Error: {}\n", msg);
			print_usage();
			return None;
		}
	}
}
