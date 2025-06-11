use common::protocol::MainToPlugin;

use crate::app_state::AppState;

use super::Command;

pub struct PluginScan {}

impl Command for PluginScan {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.sender_to_loop.send(MainToPlugin::Scan)?;
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
