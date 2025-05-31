use std::ops::Range;

use serde::{Deserialize, Serialize};

use crate::{
    model::note::Note, process_context::Event::NoteOff, process_context::Event::NoteOn,
    process_track_context::ProcessTrackContext,
};
pub mod note;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub bpm: f64,
    pub sample_rate: f64,
    pub lpb: u16,
    pub play_p: bool,
    pub play_position: Range<i64>,
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            bpm: 128.0,
            sample_rate: 48000.0,
            lpb: 4,
            play_p: false,
            play_position: (0..0),
            tracks: vec![],
        }
    }

    pub fn add_track(&mut self) {
        let mut track = Track::new();
        track.name = format!("T{:02X}", self.tracks.len() + 1);
        self.tracks.push(track);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub nlines: usize,
    pub modules: Vec<Module>,
    pub notes: Vec<Note>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            name: "T01".to_string(),
            nlines: 16,
            modules: vec![],
            notes: vec![],
        }
    }

    #[allow(dead_code)]
    pub fn note(&self, line: usize) -> Option<&Note> {
        self.notes.iter().find(|note| note.line == line)
    }

    #[allow(dead_code)]
    pub fn note_mut(&mut self, line: usize) -> Option<&mut Note> {
        self.notes.iter_mut().find(|note| note.line == line)
    }

    pub fn compute_midi(&self, context: &mut ProcessTrackContext) {
        for note in self.notes.iter() {
            let time = note.line * 0x100 + note.delay as usize;
            if context.play_position.contains(&(time as i64)) {
                if let Some(key) = context.on_key {
                    context.event_list_input.push(NoteOff(key));
                }
                // TODO time
                context
                    .event_list_input
                    .push(NoteOn(note.key, note.velocity));
                context.on_key = Some(note.key);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub id: String,
    pub state: Option<String>,
}

impl Module {
    pub fn new(id: String) -> Self {
        Self { id, state: None }
    }
}
