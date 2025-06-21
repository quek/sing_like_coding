use std::{
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};

use anyhow::Result;
use common::util::dir_user_setting;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub midi_device_input: Option<String>,
}

impl Config {
    fn file() -> PathBuf {
        dir_user_setting().join("config.json")
    }

    pub fn load() -> Result<Self> {
        let file = File::open(Self::file())?;
        let reader = BufReader::new(file);
        let config = serde_json::from_reader(reader)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let mut file = File::create(Self::file())?;
        let json = serde_json::to_string_pretty(&self)?;
        file.write_all(json.as_bytes()).unwrap();
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            midi_device_input: None,
        }
    }
}
