use anyhow::Result;

use crate::view::view_state::ViewState;

pub mod plugin_load;
pub mod plugin_scan;
pub mod track_add;

pub trait Command: Send {
    fn call(&mut self, state: &mut ViewState) -> Result<()>;
    fn name(&self) -> &str;
}
