use serde_derive::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct WindowSection {
	pub dark: bool,
	pub win_w: u32,
	pub win_h: u32,
	pub win_x: i32,
	pub win_y: i32,
}

impl Default for WindowSection {
	fn default() -> Self {
		Self { dark: false, win_w: 580, win_h: 558, win_x: 64, win_y: 64 }
	}
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct UpdateSection {
	pub check_updates: bool,
	pub last_checked: u64,
}

impl Default for UpdateSection {
	fn default() -> Self {
		Self { check_updates: true, last_checked: 0 }
	}
}

impl UpdateSection {
	pub fn should_check(&self) -> bool {
		if !self.check_updates {
			false
		} else {
			let duration = SystemTime::now()
				.duration_since(UNIX_EPOCH + Duration::from_secs(self.last_checked))
				.unwrap_or_else(|_| Duration::from_secs(0));

			duration > Duration::from_secs(60 * 60 * 24) // 24 hours
		}
	}

	pub fn set_update_check_time(&mut self) {
		self.last_checked = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_else(|_| Duration::from_secs(0))
			.as_secs();
	}
}

#[derive(Debug, Default, PartialEq, Clone, Serialize)]
pub struct Configuration {
	pub window: WindowSection,
	pub bindings: Option<BTreeMap<String, Vec<String>>>,
	pub updates: UpdateSection,
}

#[derive(Deserialize)]
pub struct IncompleteConfiguration {
	pub window: Option<WindowSection>,
	pub bindings: Option<BTreeMap<String, Vec<String>>>,
	pub updates: Option<UpdateSection>,
}

impl From<IncompleteConfiguration> for Configuration {
	fn from(cfg: IncompleteConfiguration) -> Self {
		Self {
			window: cfg.window.unwrap_or_default(),
			bindings: cfg.bindings,
			updates: cfg.updates.unwrap_or_default(),
		}
	}
}

impl Configuration {
	pub fn load<P: AsRef<Path>>(file_path: P) -> Result<Configuration, String> {
		let file_path = file_path.as_ref();
		let cfg_str = fs::read_to_string(file_path)
			.map_err(|_| format!("Could not read configuration from {:?}", file_path))?;
		let result: IncompleteConfiguration =
			toml::from_str(&cfg_str).map_err(|e| format!("{}", e))?;
		//println!("Read config from file:\n{:#?}", result);
		Ok(result.into())
	}

	pub fn save<P: AsRef<Path>>(&self, file_path: P) -> Result<(), String> {
		let file_path = file_path.as_ref();
		let string = toml::to_string(self).map_err(|e| format!("{}", e))?;
		fs::write(file_path, string)
			.map_err(|_| format!("Could not write to config file {:?}", file_path))?;
		Ok(())
	}
}
