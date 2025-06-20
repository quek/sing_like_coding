use crate::app_state::AppState;

use super::Command;

pub struct TrackAdd {}

impl Command for TrackAdd {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.track_add()?;
        Ok(())
    }

    fn name(&self) -> &str {
        "Track Add"
    }
}

impl TrackAdd {
    pub fn new() -> Self {
        Self {}
    }
}
