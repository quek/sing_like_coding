use crate::{app_state::AppState, singer::SingerMsg};

use super::Command;

pub struct TrackAdd {}

impl Command for TrackAdd {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.view_sender.send(SingerMsg::TrackAdd)?;
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
