use crate::app_state::AppState;

use super::Command;

pub struct SongSave {}

impl Command for SongSave {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.song_save()
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
