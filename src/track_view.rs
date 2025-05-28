use std::sync::mpsc::{Receiver, Sender};

use eframe::egui::Ui;

use crate::{note::note_name_to_midi, song::SongCommand};

#[derive(Debug)]
pub enum ViewCommand {
    Play,
    Stop,
    StateTrack(usize),
    Note(usize, i16),
}

pub struct TrackView {
    line_buffers: Vec<String>,
    view_sender: Sender<ViewCommand>,
    song_receiver: Receiver<SongCommand>,
    track_name: String,
}

impl TrackView {
    pub fn new(view_sender: Sender<ViewCommand>, song_receiver: Receiver<SongCommand>) -> Self {
        Self {
            line_buffers: vec![],
            view_sender,
            song_receiver,
            track_name: "".to_string(),
        }
    }

    pub fn receive_from_song(&mut self) {
        loop {
            match self.song_receiver.try_recv() {
                Ok(command) => {
                    log::debug!("View 受信 {:?}", command);
                    match command {
                        SongCommand::Track => (),
                        SongCommand::Note => (),
                        SongCommand::StateTrack(name, nlines, notes) => {
                            self.track_name = name;
                            self.line_buffers.clear();
                            for line in 0..nlines {
                                if let Some(note) = notes.iter().find(|note| note.line == line) {
                                    self.line_buffers.push(note.note_name());
                                } else {
                                    self.line_buffers.push("".to_string());
                                }
                            }
                        }
                    }
                }
                Err(_) => break,
            }
        }
    }

    pub fn view(&mut self, ui: &mut Ui) {
        self.receive_from_song();

        ui.heading(&self.track_name);
        for line in 0..self.line_buffers.len() {
            ui.horizontal(|ui| {
                ui.label(format!("{:02X}", line));
                if ui
                    .text_edit_singleline(&mut self.line_buffers[line])
                    .changed()
                {
                    log::debug!("will send ViewCommand::Note {line}");
                    note_name_to_midi(&self.line_buffers[line]).map(|key| {
                        log::debug!("will send ViewCommand::Note {line} {key}");
                        self.view_sender.send(ViewCommand::Note(line, key)).unwrap();
                    });
                }
            });
        }
    }
}
