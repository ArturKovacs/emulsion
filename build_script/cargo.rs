use serde::Deserialize;
use toml;

#[derive(Clone, Debug, Deserialize)]
pub struct Package {
	pub name: String,
	pub version: String,
	pub description: String,
	pub homepage: Option<String>,
	pub authors: Option<Vec<String>>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CargoDescriptor {
	pub package: Package,
}
