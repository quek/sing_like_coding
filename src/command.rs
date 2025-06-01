use anyhow::Result;

pub mod plugin_load;
pub mod plugin_scan;

pub trait Command: Send {
    fn call(&mut self) -> Result<()>;
    fn name(&self) -> &str;
}
