use std::sync::{Arc, Mutex};

use crate::{
    command::{self, Command},
    util::is_subsequence_case_insensitive,
};

pub struct Commander {
    pub commands: Vec<Arc<Mutex<dyn Command>>>,
}

impl Commander {
    pub fn new() -> Self {
        Self {
            commands: vec![
                Arc::new(Mutex::new(command::plugin_load::PluginLoad::new())),
                Arc::new(Mutex::new(command::plugin_scan::PluginScan::new())),
                Arc::new(Mutex::new(command::track_add::TrackAdd::new())),
                Arc::new(Mutex::new(command::song_open::SongOpen::new())),
                Arc::new(Mutex::new(command::song_save::SongSave::new())),
            ],
        }
    }

    pub fn query(&mut self, q: &str) -> Vec<Arc<Mutex<dyn Command>>> {
        self.commands
            .iter_mut()
            .filter(|x| is_subsequence_case_insensitive(x.lock().unwrap().name(), q))
            .map(|x| x.clone())
            .collect::<Vec<_>>()
    }
}
