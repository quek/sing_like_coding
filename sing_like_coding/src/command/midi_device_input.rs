use crate::{app_state::AppState, view::root_view::Route};

use super::Command;

pub struct MidiDeviceInput {}

impl Command for MidiDeviceInput {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.route = Route::MidiDeviceInputSelect;
        Ok(())
    }

    fn name(&self) -> &str {
        "Midi Device Input"
    }
}

impl MidiDeviceInput {
    pub fn new() -> Self {
        Self {}
    }
}
