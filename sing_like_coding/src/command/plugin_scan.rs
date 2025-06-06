use crate::app_state::AppState;

use super::Command;

pub struct PluginScan {}

impl Command for PluginScan {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        dbg!("PluginScan call!");
        state.clap_manager.scan();
        Ok(())
    }

    fn name(&self) -> &str {
        "Plugin Scan"
    }
}

impl PluginScan {
    pub fn new() -> Self {
        Self {}
    }
}
