use anyhow::Result;
use eframe::egui::Key;

use crate::{app_state::AppState, device::Device, singer::SingerCommand};

use super::{
    command_view::CommandView, plugin_select_view::PluginSelectView, track_view::TrackView,
};

#[derive(Debug)]
pub enum Route {
    Track,
    Command,
    PluginSelect,
}

pub struct MainView {
    track_view: TrackView,
    command_view: CommandView,
    plugin_select_view: Option<PluginSelectView>,
}

impl MainView {
    pub fn new() -> Self {
        Self {
            track_view: TrackView::new(),
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
            Route::Track => self.track_view.view(gui_context, state, device)?,
            Route::Command => self.command_view.view(gui_context, state)?,
            Route::PluginSelect => {
                if self.plugin_select_view.is_none() {
                    self.plugin_select_view = Some(PluginSelectView::new(
                        state.clap_manager.descriptions.clone(),
                    ));
                }

                if let Some(description) = self
                    .plugin_select_view
                    .as_mut()
                    .unwrap()
                    .view(gui_context)?
                {
                    // let description = description.lock().unwrap();
                    for track_index in &state.selected_tracks {
                        state
                            .view_sender
                            .send(SingerCommand::PluginLoad(
                                *track_index,
                                description.clone(),
                                state.hwnd,
                            ))
                            .unwrap();
                    }

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
        let input = gui_context.input(|i| i.clone());
        let focused = gui_context.memory(|memory| memory.focused());
        if focused.is_some() {
            return Ok(());
        }

        if input.modifiers.ctrl && input.key_pressed(eframe::egui::Key::Space) {
            state.route = Route::Command;
        } else if input.key_pressed(Key::Space) {
            state.view_sender.send(if state.song_state.play_p {
                SingerCommand::Stop
            } else {
                SingerCommand::Play
            })?;
        }

        Ok(())
    }
}
