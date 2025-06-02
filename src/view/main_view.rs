use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use anyhow::Result;
use eframe::egui::Key;

use crate::{
    clap_manager::Description,
    device::Device,
    model::song::Song,
    singer::{ClapPluginPtr, SingerMsg, SongState},
};

use super::{
    command_view::CommandView, query_view::QueryView, track_view::TrackView, view_state::ViewState,
};

#[derive(Debug)]
pub enum ViewMsg {
    #[allow(dead_code)]
    Song(Song),
    State(SongState),
    PluginCallback(ClapPluginPtr),
}

#[derive(Debug)]
pub enum Route {
    Track,
    Command,
    PluginSelect,
}

pub struct MainView {
    gui_context: Option<eframe::egui::Context>,
    state: Arc<Mutex<ViewState>>,
    track_view: TrackView,
    command_view: CommandView,
    plugin_select_view: Option<QueryView<Description>>,
    will_plugin_open: Option<(usize, usize)>,
    song_receiver: Option<Receiver<ViewMsg>>,
}

impl MainView {
    pub fn new(view_sender: Sender<SingerMsg>, song_receiver: Receiver<ViewMsg>) -> Self {
        Self {
            gui_context: None,
            state: Arc::new(Mutex::new(ViewState::new(view_sender))),
            track_view: TrackView::new(),
            command_view: CommandView::new(),
            plugin_select_view: None,
            will_plugin_open: None,
            song_receiver: Some(song_receiver),
        }
    }

    pub fn start_listener(
        &mut self,
        receiver: Receiver<ViewMsg>,
        gui_context: &eframe::egui::Context,
    ) {
        log::debug!("MainView::start_listener");
        let state = self.state.clone();
        let gui_context = gui_context.clone();
        thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                dbg!(&command);
                match command {
                    ViewMsg::Song(song) => {
                        dbg!("Song start...");
                        let mut state = state.lock().unwrap();
                        state.line_buffers.clear();
                        for track in song.tracks.iter() {
                            let mut xs = vec![];
                            for line in 0..song.nlines {
                                if let Some(note) =
                                    track.notes.iter().find(|note| note.line == line)
                                {
                                    xs.push(note.note_name());
                                } else {
                                    xs.push("".to_string());
                                }
                            }
                            state.line_buffers.push(xs);
                        }
                        state.song = song;
                        gui_context.request_repaint();
                        dbg!("Song end");
                    }
                    ViewMsg::State(song_state) => {
                        dbg!("State start...");
                        state.lock().unwrap().song_state = song_state;
                        gui_context.request_repaint();
                        dbg!("State end");
                    }
                    ViewMsg::PluginCallback(plugin) => {
                        dbg!("PluginCallbackstart start...");
                        state.lock().unwrap().callback_plugins.push(plugin);
                        dbg!("PluginCallbackstart state locked");
                        gui_context.request_repaint();
                        dbg!("PluginCallbackstart end");
                    }
                }
            }
        });
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        device: &mut Option<Device>,
    ) -> Result<()> {
        self.do_callback_plugins()?;

        dbg!("view 1");
        if let Some(receiver) = self.song_receiver.take() {
            self.start_listener(receiver, gui_context);
            self.gui_context = Some(gui_context.clone());
        }
        dbg!("view 2");
        self.process_shortcut(gui_context)?;

        dbg!("view 3");
        self.plugin_gui_open()?;

        dbg!("view 4");
        let mut state = self.state.lock().unwrap();
        dbg!("view 5");
        match &state.route {
            Route::Track => self.track_view.view(gui_context, &mut state, device)?,
            Route::Command => self.command_view.view(gui_context, &mut state)?,
            Route::PluginSelect => {
                if self.plugin_select_view.is_none() {
                    let xs = state
                        .clap_manager
                        .descriptions
                        .iter()
                        .map(|x| Arc::new(Mutex::new(x.clone())) as Arc<Mutex<Description>>)
                        .collect();
                    self.plugin_select_view = Some(QueryView::new(xs));
                }

                if let Some(description) = self
                    .plugin_select_view
                    .as_mut()
                    .unwrap()
                    .view(gui_context)?
                {
                    let description = description.lock().unwrap();
                    for track_index in &state.selected_tracks {
                        state
                            .view_sender
                            .send(SingerMsg::PluginLoad(*track_index, description.clone()))
                            .unwrap();
                        self.will_plugin_open =
                            Some((*track_index, state.song.tracks[*track_index].modules.len()));
                    }

                    self.plugin_select_view = None;
                    state.route = Route::Track;
                }
            }
        }
        Ok(())
    }

    fn do_callback_plugins(&mut self) -> Result<()> {
        dbg!("do_callback_plugins start...");
        let mut state = self.state.lock().unwrap();
        let callback_plugins = &mut state.callback_plugins;
        for plugin in callback_plugins.iter() {
            let plugin = unsafe { &*plugin.0 };
            log::debug!("will on_main_thread");
            unsafe { plugin.on_main_thread.unwrap()(plugin) };
            log::debug!("did on_main_thread");
        }
        callback_plugins.clear();
        dbg!("do_callback_plugins end");
        Ok(())
    }

    fn plugin_gui_open(&mut self) -> Result<()> {
        let plugin_ptr = if let Some((track_index, plugin_index)) = &self.will_plugin_open {
            let mut state = self.state.lock().unwrap();
            state
                .song
                .tracks
                .get_mut(*track_index)
                .map(|x| x.modules.get_mut(*plugin_index))
                .flatten()
                .map(|module| module.plugin_ptr.clone())
                .flatten()
        } else {
            None
        };

        if let Some(plugin_ptr) = plugin_ptr {
            let plugin = unsafe { plugin_ptr.as_mut() };
            dbg!("plugin.gui_open() before");
            plugin.gui_open()?;
            dbg!("plugin.gui_open() after");
            self.will_plugin_open = None;
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
