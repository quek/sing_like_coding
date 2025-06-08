use crate::app_state::AppState;

use super::Command;

pub struct SongOpen {}

impl Command for SongOpen {
    fn call(&mut self, _state: &mut AppState) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "Open"
    }
}

impl SongOpen {
    pub fn new() -> Self {
        Self {}
    }
}
