use std::{
    sync::{mpsc::Receiver, Arc, Mutex},
    thread,
};

use anyhow::Result;
use eframe::egui::Key;

use crate::{
    app_state::AppState,
    device::Device,
    model::song::Song,
    singer::{SingerMsg, SongState},
};

use super::{
    command_view::CommandView, plugin_select_view::PluginSelectView, track_view::TrackView,
};

#[derive(Debug)]
pub enum ViewMsg {
    Song(Song),
    State(SongState),
    Quit,
}

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
    song_receiver: Option<Receiver<ViewMsg>>,
}

impl MainView {
    pub fn new(app_state: Arc<Mutex<AppState>>, song_receiver: Receiver<ViewMsg>) -> Self {
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
    ) -> Result<()> {
        if let Some(receiver) = self.song_receiver.take() {
            loop_receive_from_audio_thread(self.state.clone(), receiver, gui_context);
            self.gui_context = Some(gui_context.clone());
        }

        self.process_shortcut(gui_context)?;

        let mut state = self.state.lock().unwrap();
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
                            .send(SingerMsg::PluginLoad(*track_index, description.clone()))
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
            state.view_sender.send(if state.song_state.play_p {
                SingerMsg::Stop
            } else {
                SingerMsg::Play
            })?;
        }

        Ok(())
    }
}

pub fn loop_receive_from_audio_thread(
    state: Arc<Mutex<AppState>>,
    receiver: Receiver<ViewMsg>,
    gui_context: &eframe::egui::Context,
) {
    let gui_context = gui_context.clone();
    thread::spawn(move || {
        while let Ok(command) = receiver.recv() {
            match command {
                ViewMsg::Song(song) => {
                    let mut state = state.lock().unwrap();
                    state.song = song;
                    gui_context.request_repaint();
                }
                ViewMsg::State(song_state) => {
                    state.lock().unwrap().song_state = song_state;
                    gui_context.request_repaint();
                }
                ViewMsg::Quit => return,
            }
        }
    });
}
