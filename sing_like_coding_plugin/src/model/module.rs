use serde::{Deserialize, Serialize};

use crate::{plugin::Plugin, process_track_context::PluginPtr};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub name: String,
    pub state: Option<String>,
    #[serde(skip)]
    pub plugin_ptr: Option<PluginPtr>,
}

impl Module {
    pub fn new(id: String, name: String, plugin_ptr: PluginPtr) -> Self {
        Self {
            id,
            name,
            state: None,
            plugin_ptr: Some(plugin_ptr),
        }
    }

    pub fn plugin(&mut self) -> Option<&mut Plugin> {
        self.plugin_ptr.as_ref().map(|x| unsafe { x.as_mut() })
    }
}
