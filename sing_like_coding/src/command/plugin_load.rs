use crate::{app_state::AppState, view::main_view::Route};

use super::Command;

pub struct PluginLoad {}

impl Command for PluginLoad {
    fn call(&mut self, state: &mut AppState) -> anyhow::Result<()> {
        state.route = Route::PluginSelect;
        Ok(())
    }

    fn name(&self) -> &str {
        "Plugin Load"
    }
}

impl PluginLoad {
    pub fn new() -> Self {
        Self {}
    }
}
