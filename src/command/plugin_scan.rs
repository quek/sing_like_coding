use super::Command;

pub struct PluginScan {}

impl Command for PluginScan {
    fn call(&mut self) -> anyhow::Result<()> {
        dbg!("PluginScan call!");
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
