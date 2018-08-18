use std::fs::File;
use std::path::Path;
//use std::result::Result;
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

#[derive(PartialEq, Serialize, Deserialize)]
pub struct Configuration {
    pub light_theme: bool,
    pub window_width: u32,
    pub window_height: u32,
}

impl Configuration {
    pub fn load<P: AsRef<Path>>(file_path: P) -> Result<Configuration, ()> {
        let file = File::open(file_path).map_err(|_| ())?;
        let mut de = Deserializer::new(file);

        Ok(Deserialize::deserialize(&mut de).map_err(|_| ())?)
    }

    pub fn save<P: AsRef<Path>>(&self, file_path: P) -> Result<(), ()> {
        let mut file = File::create(file_path).map_err(|_| ())?;
        let mut ser = Serializer::new(&mut file);
        self.serialize(&mut ser).map_err(|_| ())?;

        Ok(())
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            light_theme: true,
            window_width: 512,
            window_height: 512,
        }
    }
}
