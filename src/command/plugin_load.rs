use crate::view::{main_view::Route, view_state::ViewState};

use super::Command;

pub struct PluginLoad {}

impl Command for PluginLoad {
    fn call(&mut self, state: &mut ViewState) -> anyhow::Result<()> {
        dbg!("state.route = Route::PluginSelect;");
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
