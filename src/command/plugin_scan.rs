use super::Command;

#[derive(Clone)]
pub struct PluginScan {}

impl Command for PluginScan {
    fn call(&mut self) -> anyhow::Result<()> {
        dbg!("PluginScan call!");
        Ok(())
    }

    fn name(&self) -> &str {
        "Plugin Scan"
    }

    fn boxed_clone(&self) -> Box<dyn Command> {
        Box::new(self.clone())
    }
}

impl PluginScan {
    pub fn new() -> Self {
        Self {}
    }
}
