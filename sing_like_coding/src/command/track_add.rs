use crate::{app_state::AppState, singer::SingerCommand};

use super::Command;

pub struct TrackAdd {}

impl Command for TrackAdd {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.sender_to_singer.send(SingerCommand::TrackAdd)?;
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
