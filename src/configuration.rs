use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Theme {
	Light,
	Dark,
}

impl Theme {
	pub fn switch_theme(self) -> Self {
		match self {
			Theme::Dark => Theme::Light,
			Theme::Light => Theme::Dark,
		}
	}
}

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
pub struct ConfigUpdateSection {
	pub check_updates: bool,
}

impl Default for ConfigUpdateSection {
	fn default() -> Self {
		Self { check_updates: true }
	}
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct CacheUpdateSection {
	pub last_checked: u64,
}

impl Default for CacheUpdateSection {
	fn default() -> Self {
		Self { last_checked: 0 }
	}
}

impl CacheUpdateSection {
	pub fn update_check_needed(&self) -> bool {
		let duration = SystemTime::now()
			.duration_since(UNIX_EPOCH + Duration::from_secs(self.last_checked))
			.unwrap_or_else(|_| Duration::from_secs(0));

		duration > Duration::from_secs(60 * 60 * 24) // 24 hours
	}

	pub fn set_update_check_time(&mut self) {
		self.last_checked = SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.unwrap_or_else(|_| Duration::from_secs(0))
			.as_secs();
	}
}

#[derive(Debug, Default, PartialEq, Clone, Serialize)]
pub struct Cache {
	pub window: WindowSection,
	pub updates: CacheUpdateSection,
}

impl Cache {
	pub fn theme(&self) -> Theme {
		match self.window.dark {
			true => Theme::Dark,
			false => Theme::Light,
		}
	}

	pub fn set_theme(&mut self, theme: Theme) {
		self.window.dark = theme == Theme::Dark;
	}
}

#[derive(Deserialize)]
struct IncompleteCache {
	pub window: Option<WindowSection>,
	pub updates: Option<CacheUpdateSection>,
}

impl From<IncompleteCache> for Cache {
	fn from(cache: IncompleteCache) -> Self {
		Self {
			window: cache.window.unwrap_or_default(),
			updates: cache.updates.unwrap_or_default(),
		}
	}
}

impl Cache {
	pub fn load<P: AsRef<Path>>(file_path: P) -> Result<Cache, String> {
		let file_path = file_path.as_ref();
		let cfg_str = fs::read_to_string(file_path)
			.map_err(|_| format!("Could not read cache from {:?}", file_path))?;
		let result: IncompleteCache = toml::from_str(&cfg_str).map_err(|e| format!("{}", e))?;
		//println!("Read cache from file:\n{:#?}", result);
		Ok(result.into())
	}

	pub fn save<P: AsRef<Path>>(&self, file_path: P) -> Result<(), String> {
		let file_path = file_path.as_ref();
		let string = toml::to_string(self).map_err(|e| format!("{}", e))?;
		fs::write(file_path, string)
			.map_err(|_| format!("Could not write to cache file {:?}", file_path))?;
		Ok(())
	}
}

#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
pub struct EnvVar {
	pub name: String,
	pub value: String,
}

#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
pub struct Command {
	pub input: Vec<String>,
	pub program: String,
	pub args: Option<Vec<String>>,
	pub envs: Option<Vec<EnvVar>>,
}

#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
pub struct TitleSection {
	pub displayed_folders: Option<u32>,
	pub show_program_name: Option<bool>,
}

impl TitleSection {
	pub fn format_file_path<'a>(&self, file_path: &'a PathBuf) -> Cow<'a, str> {
		match self.displayed_folders {
			Some(0) | None => file_path.file_name().unwrap().to_string_lossy(),
			Some(n) => {
				let ancestor = file_path.ancestors().take(2 + n as usize).last().unwrap();
				let path = if ancestor.as_os_str().len() <= 1 {
					&file_path
				} else {
					file_path.strip_prefix(ancestor).unwrap()
				};
				path.to_string_lossy()
			}
		}
	}

	pub fn format_program_name(&self) -> &'static str {
		match self.show_program_name {
			Some(false) => "",
			_ => " : E M U L S I O N",
		}
	}
}

#[derive(Debug, Default, PartialEq, Clone, Serialize, Deserialize)]
pub struct Configuration {
	pub bindings: Option<BTreeMap<String, Vec<String>>>,
	pub commands: Option<Vec<Command>>,
	pub updates: Option<ConfigUpdateSection>,
	pub title: Option<TitleSection>,
}

impl Configuration {
	pub fn load<P: AsRef<Path>>(file_path: P) -> Result<Configuration, String> {
		let file_path = file_path.as_ref();
		let cfg_str = fs::read_to_string(file_path)
			.map_err(|_| format!("Could not read config from {:?}", file_path))?;
		let result = toml::from_str(cfg_str.as_ref()).map_err(|e| format!("{}", e))?;
		//println!("Read config from file:\n{:#?}", result);
		Ok(result)
	}
}
