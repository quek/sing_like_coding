use std::sync::{Arc, Mutex};

use eframe::egui::Ui;

use crate::{
    note::{note_name_to_midi, Note},
    song::Song,
};

pub struct TrackView {
    song: Arc<Mutex<Song>>,
    line_buffers: Vec<String>,
}

// TODO lock し過ぎ。OSC? コマンド?
impl TrackView {
    pub fn new(song: Arc<Mutex<Song>>) -> Self {
        let mut line_buffers = vec![];
        {
            let song_ref = song.lock().unwrap();
            let track = song_ref.tracks.first().unwrap();
            for line in 0..track.nlines {
                if let Some(note) = track.note(line) {
                    line_buffers.push(note.note_name());
                } else {
                    line_buffers.push("".to_string());
                }
            }
        };
        Self { song, line_buffers }
    }

    pub fn view(&mut self, ui: &mut Ui) {
        let mut song_ref = self.song.lock().unwrap();
        let track = song_ref.tracks.first_mut().unwrap();
        ui.heading(&track.name);
        for line in 0..track.nlines {
            ui.horizontal(|ui| {
                ui.label(format!("{:02X}", line));
                if ui
                    .text_edit_singleline(&mut self.line_buffers[line])
                    .changed()
                {
                    if let Some(note) = track.note_mut(line) {
                        note.set_note_name(&self.line_buffers[line]);
                    } else {
                        note_name_to_midi(&self.line_buffers[line]).map(|key| {
                            track.notes.push(Note {
                                line,
                                delay: 0,
                                channel: 0,
                                key,
                                velocity: 100.0,
                            })
                        });
                    }
                }
            });
        }
    }
}
