use crate::app_state::AppState;

use super::Command;

pub struct SongSave {}

impl Command for SongSave {
    fn call(&mut self, _state: &mut AppState) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "Save"
    }
}

impl SongSave {
    pub fn new() -> Self {
        Self {}
    }
}
