use serde::{Deserialize, Serialize};

use crate::app_state::Cursor;

use super::{note::Note, track::Track};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub bpm: f64,
    pub sample_rate: f64,
    pub lpb: u16,
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            bpm: 128.0,
            sample_rate: 48000.0,
            lpb: 4,
            tracks: vec![],
        }
    }

    pub fn add_track(&mut self) {
        let mut track = Track::new();
        track.name = format!("T{:02X}", self.tracks.len() + 1);
        self.tracks.push(track);
    }

    pub fn note(&self, cursor: &Cursor) -> Option<&Note> {
        self.tracks
            .get(cursor.track)
            .and_then(|x| x.lanes.get(cursor.lane))
            .and_then(|x| x.note(cursor.line))
    }
}
