use anyhow::Result;

use crate::app_state::AppState;

pub mod midi_device_input;
pub mod plugin_load;
pub mod plugin_scan;
pub mod song_open;
pub mod song_save;
pub mod track_add;

pub trait Command: Send {
    fn call(&mut self, state: &mut AppState) -> Result<()>;
    fn name(&self) -> &str;
}
