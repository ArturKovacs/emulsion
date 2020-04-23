use std::fs;
use std::path::Path;
use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Configuration {
	pub dark: bool,
	pub win_w: u32,
	pub win_h: u32,
	pub win_x: i32,
	pub win_y: i32,
    pub bindings: BTreeMap<String, Vec<String>>,
}

impl Configuration {
	pub fn load<P: AsRef<Path>>(file_path: P) -> Result<Configuration, String> {
		let file_path = file_path.as_ref();
		let cfg_str = fs::read_to_string(file_path)
			.map_err(|_| format!("Could not read configuration from {:?}", file_path))?;
		let result = toml::from_str(cfg_str.as_ref()).map_err(|e| format!("{}", e))?;
		println!("Read config from file:\n{:#?}", result);
		Ok(result)
		//let file = fs::File::open(file_path).map_err(|_| ())?;
		//let mut de = Deserializer::new(file);
		//Ok(Deserialize::deserialize(&mut de).map_err(|_| ())?)
	}

	pub fn save<P: AsRef<Path>>(&self, file_path: P) -> Result<(), String> {
		let file_path = file_path.as_ref();
		let string = toml::to_string(self).map_err(|e| format!("{}", e))?;
		fs::write(file_path, string)
			.map_err(|_| format!("Could not write to config file {:?}", file_path))?;
		//let mut ser = Serializer::new(&mut file);
		//self.serialize(&mut ser).map_err(|_| ())?;
		Ok(())
	}
}

impl Default for Configuration {
	fn default() -> Self {
		Configuration {
			dark: false,
			win_w: 580,
			win_h: 558,
			win_x: 64,
			win_y: 64,
            bindings: BTreeMap::new(),
		}
	}
}
