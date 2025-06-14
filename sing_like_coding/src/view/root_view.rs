use anyhow::Result;
use eframe::egui::{ahash::HashMap, Key};

use crate::{
    app_state::{AppState, UiCommand},
    device::Device,
    singer::SingerCommand,
};

use super::{
    command_view::CommandView,
    main_view::MainView,
    plugin_select_view::PluginSelectView,
    shortcut_key::{shortcut_key, Modifier},
};

#[derive(Debug)]
pub enum Route {
    Track,
    Command,
    PluginSelect,
}

pub struct RootView {
    shortcut_map: HashMap<(Modifier, Key), UiCommand>,
    main_view: MainView,
    command_view: CommandView,
    plugin_select_view: Option<PluginSelectView>,
}

impl RootView {
    pub fn new() -> Self {
        let shortcut_map = [
            ((Modifier::None, Key::Space), UiCommand::PlayToggle),
            ((Modifier::C, Key::Space), UiCommand::Command),
            ((Modifier::None, Key::V), UiCommand::FocusedPartNext),
            ((Modifier::S, Key::V), UiCommand::FocusedPartPrev),
        ];

        let shortcut_map: HashMap<_, _> = shortcut_map.into_iter().collect();

        Self {
            shortcut_map,
            main_view: MainView::new(),
            command_view: CommandView::new(),
            plugin_select_view: None,
        }
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        device: &mut Option<Device>,
        state: &mut AppState,
    ) -> Result<()> {
        self.process_shortcut(state, gui_context)?;

        state.receive_from_singer()?;
        state.receive_from_communicator()?;

        match &state.route {
            Route::Track => self.main_view.view(gui_context, state, device)?,
            Route::Command => self.command_view.view(gui_context, state)?,
            Route::PluginSelect => {
                if self.plugin_select_view.is_none() {
                    self.plugin_select_view = Some(PluginSelectView::new());
                }

                if let Some(description) = self
                    .plugin_select_view
                    .as_mut()
                    .unwrap()
                    .view(gui_context)?
                {
                    state.sender_to_singer.send(SingerCommand::PluginLoad(
                        state.track_state.index,
                        description.id,
                        description.name,
                    ))?;

                    self.plugin_select_view = None;
                    state.route = Route::Track;
                }
            }
        }
        Ok(())
    }

    fn process_shortcut(
        &mut self,
        state: &mut AppState,
        gui_context: &eframe::egui::Context,
    ) -> Result<()> {
        let focused = gui_context.memory(|memory| memory.focused());
        if focused.is_some() {
            return Ok(());
        }

        if let Some(key) = shortcut_key(gui_context) {
            if let Some(command) = self.shortcut_map.get(&key) {
                state.run_ui_command(command)?;
            }
        }

        Ok(())
    }
}
