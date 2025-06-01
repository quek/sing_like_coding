use super::Command;

pub struct PluginLoad {}

impl Command for PluginLoad {
    fn call(&mut self) -> anyhow::Result<()> {
        dbg!("PluginLoad call!");
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
