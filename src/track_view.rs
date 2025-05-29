use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use anyhow::Result;
use eframe::egui::{Color32, TextEdit, Ui};

use crate::{
    model::{note::note_name_to_midi, Song},
    singer::{ClapPluginPtr, Singer, SongCommand, SongState},
};

#[derive(Debug)]
pub enum ViewCommand {
    #[allow(dead_code)]
    Play,
    #[allow(dead_code)]
    Stop,
    Song,
    Note(usize, usize, i16),
    NoteOn(usize, i16, i16, f64, u32),
    NoteOff(usize, i16, i16, f64, u32),
    PluginLoad(usize, String),
    TrackAdd,
}

pub struct TrackView {
    line_buffers: Vec<Vec<String>>,
    view_sender: Sender<ViewCommand>,
    gui_context: Option<eframe::egui::Context>,
    song_state: SongState,
    callback_plugins: Vec<ClapPluginPtr>,
    song: Song,
}

impl TrackView {
    pub fn new(view_sender: Sender<ViewCommand>) -> Self {
        Self {
            line_buffers: vec![],
            view_sender,
            gui_context: None,
            song_state: SongState::default(),
            callback_plugins: vec![],
            song: Song::new(),
        }
    }

    pub fn start_listener(view: Arc<Mutex<Self>>, receiver: Receiver<SongCommand>) {
        log::debug!("TrackView::start_listener");
        thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    SongCommand::Track => (),
                    SongCommand::Song(song) => {
                        let mut view = view.lock().unwrap();
                        view.line_buffers.clear();
                        for track in song.tracks.iter() {
                            let mut xs = vec![];
                            for line in 0..track.nlines {
                                if let Some(note) =
                                    track.notes.iter().find(|note| note.line == line)
                                {
                                    xs.push(note.note_name());
                                } else {
                                    xs.push("".to_string());
                                }
                            }
                            view.line_buffers.push(xs);
                        }
                        view.song = song;
                        view.gui_context.as_ref().map(|x| x.request_repaint());
                    }
                    SongCommand::State(song_state) => {
                        let mut view = view.lock().unwrap();
                        view.song_state = song_state;
                        view.gui_context.as_ref().map(|x| x.request_repaint());
                    }
                    SongCommand::PluginCallback(plugin) => {
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
        ui: &mut Ui,
        gui_context: &eframe::egui::Context,
        singer: &Arc<Mutex<Singer>>,
    ) -> Result<()> {
        if self.gui_context.is_none() {
            self.gui_context = Some(gui_context.clone());
        }

        for plugin in self.callback_plugins.iter() {
            let plugin = unsafe { &*plugin.0 };
            log::debug!("will on_main_thread");
            unsafe { plugin.on_main_thread.unwrap()(plugin) };
            log::debug!("did on_main_thread");
        }
        self.callback_plugins.clear();

        ui.label(format!("line {}", self.song_state.line_play));
        if ui.button("Play").clicked() {
            self.view_sender.send(ViewCommand::Play).unwrap();
        }
        if ui.button("Stop").clicked() {
            self.view_sender.send(ViewCommand::Stop).unwrap();
        }

        if ui.button("Load Surge XT").clicked() {
            let path =
                "c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap".to_string();
            let track_index = 0;
            self.view_sender
                .send(ViewCommand::PluginLoad(track_index, path))
                .unwrap();
        }

        if ui.button("Load VCV Rack 2").clicked() {
            let path = "c:/Program Files/Common Files/CLAP/VCV Rack 2.clap".to_string();
            let track_index = 0;
            self.view_sender
                .send(ViewCommand::PluginLoad(track_index, path))
                .unwrap();
        }

        if ui.button("Load TyrellN6").clicked() {
            let path = "c:/Program Files/Common Files/CLAP/u-he/TyrellN6.clap".to_string();
            let track_index = 0;
            self.view_sender
                .send(ViewCommand::PluginLoad(track_index, path))
                .unwrap();
        }

        if ui.button("Load Zebralette3").clicked() {
            let path = "c:/Program Files/Common Files/CLAP/u-he/Zebralette3.clap".to_string();
            let track_index = 0;
            self.view_sender
                .send(ViewCommand::PluginLoad(track_index, path))
                .unwrap();
        }

        if ui.button("Open").clicked() {
            // main thread で処理しないといけないので、send せずに実装
            log::debug!("Open before lock");
            let mut singer = singer.lock().unwrap();
            log::debug!("Open after lock");
            let plugin = &mut singer.plugins[0][0];
            log::debug!("Open plugin");
            plugin.gui_open().unwrap();
            log::debug!("did gui_open");
        }

        ui.separator();

        if ui.button("Note On").clicked() {
            let track_index = 0;
            let key = 63;
            let channel = 0;
            let velocity = 100.0;
            let time = 0;
            self.view_sender
                .send(ViewCommand::NoteOn(
                    track_index,
                    key,
                    channel,
                    velocity,
                    time,
                ))
                .unwrap();
        }

        if ui.button("Note Off").clicked() {
            let track_index = 0;
            let key = 63;
            let channel = 0;
            let velocity = 0.0;
            let time = 0;
            self.view_sender
                .send(ViewCommand::NoteOff(
                    track_index,
                    key,
                    channel,
                    velocity,
                    time,
                ))
                .unwrap();
        }

        ui.separator();

        if ui.button("Add Track").clicked() {
            self.view_sender.send(ViewCommand::TrackAdd)?;
        }

        let nlines = self.song.tracks.first().map(|x| x.nlines).unwrap_or(0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(format!("{:02X}", nlines));
                for line in 0..nlines {
                    ui.label(format!("{:02X}", line));
                }
            });
            for (track_index, (track, line_buffer)) in self
                .song
                .tracks
                .iter()
                .zip(self.line_buffers.iter_mut())
                .enumerate()
            {
                ui.vertical(|ui| {
                    ui.heading(&track.name);
                    for line in 0..track.nlines {
                        let text_edit = TextEdit::singleline(&mut line_buffer[line]);
                        let text_edit = text_edit.desired_width(30.0);
                        let text_edit = if line == self.song_state.line_play % 0x0f {
                            text_edit.background_color(Color32::GREEN)
                        } else {
                            text_edit
                        };
                        if ui.add(text_edit).changed() {
                            note_name_to_midi(&line_buffer[line]).map(|key| {
                                self.view_sender
                                    .send(ViewCommand::Note(track_index, line, key))
                                    .unwrap();
                            });
                        }
                    }
                });
            }
        });

        Ok(())
    }
}
