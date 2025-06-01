use crate::{
    command::{plugin_scan::PluginScan, Command},
    util::is_subsequence_case_insensitive,
};

pub struct Commander {
    pub commands: Vec<Box<dyn Command>>,
}

impl Commander {
    pub fn new() -> Self {
        Self {
            commands: vec![Box::new(PluginScan::new())],
        }
    }

    pub fn query(&mut self, q: &str) -> Vec<Box<dyn Command>> {
        self.commands
            .iter_mut()
            .filter(|x| is_subsequence_case_insensitive(x.name(), q))
            .map(|x| x.boxed_clone())
            .collect::<Vec<_>>()
    }
}
