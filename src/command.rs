use anyhow::Result;

pub mod plugin_scan;

pub trait Command: Send + Sync {
    fn boxed_clone(&self) -> Box<dyn Command>;
    fn call(&mut self) -> Result<()>;
    fn name(&self) -> &str;
}
