use serde::{Deserialize, Serialize};

use crate::process_track_context::PluginPtr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub state: Option<String>,
    #[serde(skip)]
    pub plugin: Option<PluginPtr>,
}

impl Module {
    pub fn new(id: String, plugin: PluginPtr) -> Self {
        Self {
            id,
            state: None,
            plugin: Some(plugin),
        }
    }
}
