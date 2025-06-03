use std::sync::{Arc, Mutex};

use crate::{
    command::{plugin_load::PluginLoad, plugin_scan::PluginScan, track_add::TrackAdd, Command},
    util::is_subsequence_case_insensitive,
};

pub struct Commander {
    pub commands: Vec<Arc<Mutex<dyn Command>>>,
}

impl Commander {
    pub fn new() -> Self {
        Self {
            commands: vec![
                Arc::new(Mutex::new(PluginLoad::new())),
                Arc::new(Mutex::new(PluginScan::new())),
                Arc::new(Mutex::new(TrackAdd::new())),
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
