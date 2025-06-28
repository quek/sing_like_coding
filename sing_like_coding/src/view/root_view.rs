use anyhow::Result;
use common::{
    module::AudioInput,
    protocol::{MainToPlugin, PluginToMain},
};
use eframe::egui::{ahash::HashMap, Key};

use crate::{
    app_state::{AppState, UiCommand},
    device::Device,
    midi_device::MidiDevice,
    view::param_select_view::ReturnState,
};

use super::{
    command_view::CommandView,
    eval_window::EvalWindow,
    main_view::MainView,
    param_select_view::ParamSelectView,
    plugin_select_view::{self, PluginSelectView},
    select_view::{self, SelectItem, SelectView},
    shortcut_key::{shortcut_key, Modifier},
    sidechain_select_view::{self, SidechainSelectView},
};

#[derive(Debug)]
pub enum Route {
    Track,
    Command,
    MidiDeviceInputSelect,
    PluginSelect,
    ParamSelect,
    SidechainSelect,
}

#[derive(Clone)]
struct MidiPort {
    pub name: String,
}

impl SelectItem for MidiPort {
    fn name(&self) -> &str {
        &self.name
    }
}

pub struct RootView {
    eval_window: EvalWindow,
    shortcut_map: HashMap<(Modifier, Key), UiCommand>,
    main_view: MainView,
    command_view: CommandView,
    midi_device_input_select_view: Option<SelectView<MidiPort>>,
    param_select_view: Option<ParamSelectView>,
    plugin_select_view: Option<PluginSelectView>,
    sidechain_select_view: Option<SidechainSelectView>,
}

impl RootView {
    pub fn new() -> Self {
        let shortcut_map = [
            ((Modifier::None, Key::Space), UiCommand::PlayToggle),
            ((Modifier::C, Key::Space), UiCommand::PlayCursor),
            ((Modifier::None, Key::Slash), UiCommand::Command),
            ((Modifier::None, Key::V), UiCommand::FocusedPartNext),
            ((Modifier::S, Key::V), UiCommand::FocusedPartPrev),
            ((Modifier::None, Key::Q), UiCommand::EvalWindowOpen),
        ];

        let shortcut_map: HashMap<_, _> = shortcut_map.into_iter().collect();

        Self {
            eval_window: EvalWindow::new(),
            shortcut_map,
            main_view: MainView::new(),
            command_view: CommandView::new(),
            midi_device_input_select_view: None,
            param_select_view: None,
            plugin_select_view: None,
            sidechain_select_view: None,
        }
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        device: &mut Option<Device>,
        state: &mut AppState,
    ) -> Result<()> {
        state.song_next_apply()?;
        self.process_shortcut(state, gui_context)?;

        if state.eval_window_open_p {
            self.eval_window.view(gui_context, state)?;
        }

        state.receive_from_communicator()?;

        match &state.route {
            Route::Track => self.main_view.view(gui_context, state, device)?,
            Route::Command => self.command_view.view(gui_context, state)?,
            Route::MidiDeviceInputSelect => {
                self.midi_device_input_select_view(gui_context, state)?
            }
            Route::ParamSelect => self.param_select_view(gui_context, state)?,
            Route::PluginSelect => self.plugin_select_view(gui_context, state)?,
            Route::SidechainSelect => self.sidechain_select_view(gui_context, state)?,
        }

        Ok(())
    }

    fn midi_device_input_select_view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut AppState,
    ) -> Result<()> {
        let view = self.midi_device_input_select_view.get_or_insert_with(|| {
            let items = MidiDevice::list()
                .into_iter()
                .map(|name| MidiPort { name })
                .collect();
            SelectView::<MidiPort>::new(items)
        });

        match view.view(gui_context)? {
            select_view::ReturnState::Selected(item) => {
                state.midi_device_input_open(&item.name)?;
                self.midi_device_input_select_view = None;
                state.route = Route::Track;
            }
            select_view::ReturnState::Continue => {}
            select_view::ReturnState::Cancel => {
                self.midi_device_input_select_view = None;
                state.route = Route::Track;
            }
        }
        Ok(())
    }

    fn param_select_view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut AppState,
    ) -> Result<()> {
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
                        MainToPlugin::Params(
                            state
                                .song
                                .module_at((state.cursor_track.track, module_index))
                                .unwrap()
                                .id,
                        ),
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
        Ok(())
    }

    fn plugin_select_view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut AppState,
    ) -> Result<()> {
        let plugin_select_view = self
            .plugin_select_view
            .get_or_insert_with(|| PluginSelectView::new());

        match plugin_select_view.view(gui_context)? {
            plugin_select_view::ReturnState::Selected(description) => {
                state.plugin_load(&description, !gui_context.input(|i| i.modifiers.shift))?;
                self.plugin_select_view = None;
                state.route = Route::Track;
            }
            plugin_select_view::ReturnState::Continue => {}
            plugin_select_view::ReturnState::Cancel => {
                self.plugin_select_view = None;
                state.route = Route::Track;
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

    fn sidechain_select_view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut AppState,
    ) -> Result<()> {
        let view = self.sidechain_select_view.get_or_insert_with(|| {
            let cursor_track_index = state.cursor_track.track;
            let cursor_module_index = state.cursor_module.index;
            let items = state
                .song
                .tracks
                .iter()
                .enumerate()
                .flat_map(|(track_index, track)| {
                    track
                        .modules
                        .iter()
                        .enumerate()
                        .filter(move |(module_index, _module)| {
                            track_index != cursor_track_index || *module_index < cursor_module_index
                        })
                        .map(move |(module_index, module)| sidechain_select_view::Item {
                            name: format!("{} {}", track.name, module.name),
                            module_index: (track_index, module_index),
                        })
                })
                .collect();
            SidechainSelectView::new(items)
        });

        match view.view(gui_context)? {
            sidechain_select_view::ReturnState::Selected(item) => {
                let audio_input = AudioInput {
                    src_module_index: item.module_index,
                    src_port_index: 0,
                    dst_port_index: 1,
                };
                state.plugin_sidechain(state.module_index_at_cursor(), audio_input)?;
                self.sidechain_select_view = None;
                state.route = Route::Track;
            }
            sidechain_select_view::ReturnState::Continue => {}
            sidechain_select_view::ReturnState::Cancel => {
                self.sidechain_select_view = None;
                state.route = Route::Track;
            }
        }
        Ok(())
    }
}
