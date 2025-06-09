use std::sync::{mpsc::Receiver, Arc, Mutex};

use anyhow::Result;
use eframe::egui::Key;

use crate::{
    app_state::{loop_receive_from_audio_thread, AppState, AppStateCommand},
    device::Device,
    singer::SingerCommand,
    song_state::SongState,
};

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
    gui_context: Option<eframe::egui::Context>,
    state: Arc<Mutex<AppState>>,
    track_view: TrackView,
    command_view: CommandView,
    plugin_select_view: Option<PluginSelectView>,
    song_receiver: Option<Receiver<AppStateCommand>>,
}

impl MainView {
    pub fn new(app_state: Arc<Mutex<AppState>>, song_receiver: Receiver<AppStateCommand>) -> Self {
        Self {
            gui_context: None,
            state: app_state,
            track_view: TrackView::new(),
            command_view: CommandView::new(),
            plugin_select_view: None,
            song_receiver: Some(song_receiver),
        }
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        device: &mut Option<Device>,
        song_state: &SongState,
    ) -> Result<()> {
        if let Some(receiver) = self.song_receiver.take() {
            loop_receive_from_audio_thread(self.state.clone(), receiver, gui_context);
            self.gui_context = Some(gui_context.clone());
        }

        self.process_shortcut(gui_context)?;

        let mut state = self.state.lock().unwrap();
        state.receive_communicator_to_main_thread()?;

        match &state.route {
            Route::Track => self.track_view.view(gui_context, &mut state, device)?,
            Route::Command => self.command_view.view(gui_context, &mut state)?,
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

    fn process_shortcut(&mut self, gui_context: &eframe::egui::Context) -> Result<()> {
        let input = gui_context.input(|i| i.clone());
        let focused = gui_context.memory(|memory| memory.focused());
        if focused.is_some() {
            return Ok(());
        }

        let mut state = self.state.lock().unwrap();

        if input.modifiers.ctrl && input.key_pressed(eframe::egui::Key::Space) {
            state.route = Route::Command;
        } else if input.key_pressed(Key::Space) {
            state.view_sender.send(if state.xsong_state.play_p {
                SingerCommand::Stop
            } else {
                SingerCommand::Play
            })?;
        }

        Ok(())
    }
}
