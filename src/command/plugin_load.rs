use crate::view::view_state::ViewState;

use super::Command;

pub struct PluginLoad {}

impl Command for PluginLoad {
    fn call(&mut self, _state: &mut ViewState) -> anyhow::Result<()> {
        dbg!("PluginScan Load!");
        Ok(())
    }

    fn name(&self) -> &str {
        "Plugin Load"
    }
}

impl PluginLoad {
    pub fn new() -> Self {
        Self {}
    }
}
