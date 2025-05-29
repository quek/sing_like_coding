use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use eframe::egui::{Color32, TextEdit, Ui};

use crate::{
    model::note::note_name_to_midi,
    singer::{Singer, SongCommand, SongState},
};

#[derive(Debug)]
pub enum ViewCommand {
    #[allow(dead_code)]
    Play,
    #[allow(dead_code)]
    Stop,
    Song,
    Note(usize, i16),
    NoteOn(usize, i16, i16, f64, u32),
    NoteOff(usize, i16, i16, f64, u32),
    PluginLoad(usize, String),
}

pub struct TrackView {
    line_buffers: Vec<String>,
    view_sender: Sender<ViewCommand>,
    track_name: String,
    gui_context: Option<eframe::egui::Context>,
    song_state: SongState,
}

impl TrackView {
    pub fn new(view_sender: Sender<ViewCommand>) -> Self {
        Self {
            line_buffers: vec![],
            view_sender,
            track_name: "".to_string(),
            gui_context: None,
            song_state: SongState::default(),
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
                        let track = &song.tracks[0];
                        view.track_name = track.name.clone();
                        view.line_buffers.clear();
                        for line in 0..track.nlines {
                            if let Some(note) = track.notes.iter().find(|note| note.line == line) {
                                view.line_buffers.push(note.note_name());
                            } else {
                                view.line_buffers.push("".to_string());
                            }
                        }
                        view.gui_context.as_ref().map(|x| x.request_repaint());
                    }
                    SongCommand::State(song_state) => {
                        let mut view = view.lock().unwrap();
                        view.song_state = song_state;
                        view.gui_context.as_ref().map(|x| x.request_repaint());
                    }
                    SongCommand::PluginCallback(plugin) => {
                        let plugin = unsafe { &*plugin };
                        log::debug!("will on_main_thread");
                        unsafe { plugin.on_main_thread.unwrap()(plugin) };
                        log::debug!("did on_main_thread");
                        let mut view = view.lock().unwrap();
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
    ) {
        if self.gui_context.is_none() {
            self.gui_context = Some(gui_context.clone());
        }

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

        ui.heading(&self.track_name);
        let nlines = self.line_buffers.len();
        for line in 0..nlines {
            ui.horizontal(|ui| {
                ui.label(format!("{:02X}", line));

                let text_edit = TextEdit::singleline(&mut self.line_buffers[line]);
                let text_edit = if line == self.song_state.line_play % 0x0f {
                    text_edit.background_color(Color32::GREEN)
                } else {
                    text_edit
                };
                if ui.add(text_edit).changed() {
                    note_name_to_midi(&self.line_buffers[line]).map(|key| {
                        self.view_sender.send(ViewCommand::Note(line, key)).unwrap();
                    });
                }
            });
        }
    }
}
