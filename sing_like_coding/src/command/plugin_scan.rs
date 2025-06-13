use common::protocol::MainToPlugin;

use crate::app_state::AppState;

use super::Command;

pub struct PluginScan {}

impl Command for PluginScan {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.send_to_plugin(MainToPlugin::Scan, Box::new(|_, _| Ok(())))?;
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
