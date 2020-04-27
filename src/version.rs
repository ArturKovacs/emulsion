use std::fmt;
use std::{num::ParseIntError, str::FromStr};

/// A semver version
#[derive(Debug, Default, PartialEq, PartialOrd, Eq, Ord)]
pub struct Version {
	major: u32,
	minor: u32,
	patch: u32,
}

impl Version {
	/// Return the version of the cargo build
	pub fn cargo_pkg_version() -> Self {
		let major = env!("CARGO_PKG_VERSION_MAJOR").parse().expect("Invalid cargo version");
		let minor = env!("CARGO_PKG_VERSION_MINOR").parse().expect("Invalid cargo version");
		let patch = env!("CARGO_PKG_VERSION_PATCH").parse().expect("Invalid cargo version");

		Version { major, minor, patch }
	}
}

impl fmt::Display for Version {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
	}
}

impl FromStr for Version {
	type Err = ParseIntError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		// Trim letters from the start of the version tag, e.g. "v1.9" -> "1.9"
		let mut iter = s.trim_start_matches(char::is_alphabetic).split('.');
		// Consume the next part of the version tag
		let mut extract_part = || iter.next().filter(|&n| !n.is_empty()).unwrap_or("0");

		let major = extract_part().parse()?;
		let minor = extract_part().parse()?;
		let patch = extract_part().parse()?;

		Ok(Version { major, minor, patch })
	}
}
