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
    singer::{ClapPluginPtr, Singer, SingerMsg, SongState},
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
    callback_plugins: Vec<ClapPluginPtr>,
    state: ViewState,
    track_view: TrackView,
    command_view: CommandView,
    plugin_select_view: Option<QueryView<Description>>,
}

impl MainView {
    pub fn new(view_sender: Sender<SingerMsg>) -> Self {
        Self {
            gui_context: None,
            callback_plugins: vec![],
            state: ViewState::new(view_sender),
            track_view: TrackView::new(),
            command_view: CommandView::new(),
            plugin_select_view: None,
        }
    }

    pub fn start_listener(view: Arc<Mutex<Self>>, receiver: Receiver<ViewMsg>) {
        log::debug!("MainView::start_listener");
        thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    ViewMsg::Song(song) => {
                        let mut view = view.lock().unwrap();
                        view.state.line_buffers.clear();
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
                            view.state.line_buffers.push(xs);
                        }
                        view.state.song = song;
                        view.gui_context.as_ref().map(|x| x.request_repaint());
                    }
                    ViewMsg::State(song_state) => {
                        let mut view = view.lock().unwrap();
                        view.state.song_state = song_state;
                        view.gui_context.as_ref().map(|x| x.request_repaint());
                    }
                    ViewMsg::PluginCallback(plugin) => {
                        let mut view = view.lock().unwrap();
                        view.callback_plugins.push(plugin);
                        view.gui_context.as_ref().map(|x| x.request_repaint());
                    }
                }
            }
        });
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        device: &mut Option<Device>,
        singer: &Arc<Mutex<Singer>>,
    ) -> Result<()> {
        for plugin in self.callback_plugins.iter() {
            let plugin = unsafe { &*plugin.0 };
            log::debug!("will on_main_thread");
            unsafe { plugin.on_main_thread.unwrap()(plugin) };
            log::debug!("did on_main_thread");
        }
        self.callback_plugins.clear();

        if self.gui_context.is_none() {
            self.gui_context = Some(gui_context.clone());
        }
        self.process_shortcut(gui_context)?;

        dbg!(&self.state.route);
        match &self.state.route {
            Route::Track => self
                .track_view
                .view(gui_context, &mut self.state, device, singer)?,
            Route::Command => self.command_view.view(gui_context, &mut self.state)?,
            Route::PluginSelect => {
                dbg!("Route::PluginSelect");
                if self.plugin_select_view.is_none() {
                    let xs = self
                        .state
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
                    let path = &description.path;
                    let index = description.index;
                    for track_index in &self.state.selected_tracks {
                        self.state
                            .view_sender
                            .send(SingerMsg::PluginLoad(*track_index, path.clone(), index))
                            .unwrap();
                    }

                    self.plugin_select_view = None;
                    self.state.route = Route::Track;
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

        if input.modifiers.ctrl && input.key_pressed(eframe::egui::Key::Space) {
            self.state.route = Route::Command;
        } else if input.key_pressed(Key::Space) {
            self.state
                .view_sender
                .send(if self.state.song_state.play_p {
                    SingerMsg::Stop
                } else {
                    SingerMsg::Play
                })?;
        }

        Ok(())
    }
}
