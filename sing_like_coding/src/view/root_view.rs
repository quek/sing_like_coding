use anyhow::Result;
use common::protocol::{MainToPlugin, PluginToMain};
use eframe::egui::{ahash::HashMap, Key};

use crate::{
    app_state::{AppState, UiCommand},
    device::Device,
    singer::SingerCommand,
    view::param_select_view::ReturnState,
};

use super::{
    command_view::CommandView,
    main_view::MainView,
    param_select_view::ParamSelectView,
    plugin_select_view::{self, PluginSelectView},
    shortcut_key::{shortcut_key, Modifier},
};

#[derive(Debug)]
pub enum Route {
    Track,
    Command,
    PluginSelect,
    ParamSelect,
}

pub struct RootView {
    shortcut_map: HashMap<(Modifier, Key), UiCommand>,
    main_view: MainView,
    command_view: CommandView,
    param_select_view: Option<ParamSelectView>,
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
            param_select_view: None,
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
            Route::ParamSelect => {
                if state.song_state.param_track_index == state.cursor_track.track {
                    self.param_select_view = None;
                    state.route = Route::Track;
                    state.param_set(
                        state.song_state.param_module_index,
                        state.song_state.param_id,
                    )?;
                } else {
                    let param_select_view = self
                        .param_select_view
                        .get_or_insert_with(|| ParamSelectView::new());
                    match param_select_view.view(
                        gui_context,
                        &state.song.tracks[state.cursor_track.track].modules,
                        &state.param_select_view_params,
                    )? {
                        ReturnState::Selected(module_index, param) => {
                            self.param_select_view = None;
                            state.route = Route::Track;
                            state.param_set(module_index, param.id)?;
                        }
                        ReturnState::Params(module_index) => {
                            let callback: Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>> =
                                Box::new(|state, command| {
                                    if let PluginToMain::DidParams(params) = command {
                                        state.param_select_view_params = params;
                                    }
                                    Ok(())
                                });
                            state.send_to_plugin(
                                MainToPlugin::Params(state.cursor_track.track, module_index),
                                callback,
                            )?;
                        }
                        ReturnState::Continue => {}
                        ReturnState::Cancel => {
                            self.param_select_view = None;
                            state.route = Route::Track;
                        }
                    }
                }
            }
            Route::PluginSelect => {
                let plugin_select_view = self
                    .plugin_select_view
                    .get_or_insert_with(|| PluginSelectView::new());

                match plugin_select_view.view(gui_context)? {
                    plugin_select_view::ReturnState::Selected(description) => {
                        state.sender_to_singer.send(SingerCommand::PluginLoad(
                            state.cursor_track.track,
                            description.id,
                            description.name,
                        ))?;

                        self.plugin_select_view = None;
                        state.route = Route::Track;
                    }
                    plugin_select_view::ReturnState::Continue => {}
                    plugin_select_view::ReturnState::Cancel => {
                        self.plugin_select_view = None;
                        state.route = Route::Track;
                    }
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
