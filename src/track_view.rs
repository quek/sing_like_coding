use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use eframe::egui::Ui;

use crate::{model::note::note_name_to_midi, singer::SongCommand};

#[derive(Debug)]
pub enum ViewCommand {
    #[allow(dead_code)]
    Play,
    #[allow(dead_code)]
    Stop,
    StateTrack(usize),
    Note(usize, i16),
    NoteOn(usize, i16, i16, f64, u32),
    NoteOff(usize, i16, i16, f64, u32),
    LoadPlugin(usize, String),
}

pub struct TrackView {
    line_buffers: Vec<String>,
    view_sender: Sender<ViewCommand>,
    track_name: String,
    gui_context: Option<eframe::egui::Context>,
}

impl TrackView {
    pub fn new(view_sender: Sender<ViewCommand>) -> Self {
        Self {
            line_buffers: vec![],
            view_sender,
            track_name: "".to_string(),
            gui_context: None,
        }
    }

    pub fn start_listener(view: Arc<Mutex<Self>>, receiver: Receiver<SongCommand>) {
        log::debug!("TrackView::start_listener");
        thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    SongCommand::Track => (),
                    SongCommand::Note => (),
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
                    }
                }
            }
        });
    }

    pub fn view(&mut self, ui: &mut Ui, gui_context: &eframe::egui::Context) {
        if self.gui_context.is_none() {
            self.gui_context = Some(gui_context.clone());
        }

        if ui.button("Load Surge XT").clicked() {
            let path =
                "c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap".to_string();
            let track_index = 0;
            self.view_sender
                .send(ViewCommand::LoadPlugin(track_index, path))
                .unwrap();
        }

        if ui.button("Load VCV Rack 2").clicked() {
            let path = "c:/Program Files/Common Files/CLAP/VCV Rack 2.clap".to_string();
            let track_index = 0;
            self.view_sender
                .send(ViewCommand::LoadPlugin(track_index, path))
                .unwrap();
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
        for line in 0..self.line_buffers.len() {
            ui.horizontal(|ui| {
                ui.label(format!("{:02X}", line));
                if ui
                    .text_edit_singleline(&mut self.line_buffers[line])
                    .changed()
                {
                    note_name_to_midi(&self.line_buffers[line]).map(|key| {
                        self.view_sender.send(ViewCommand::Note(line, key)).unwrap();
                    });
                }
            });
        }
    }
}
