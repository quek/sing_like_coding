use serde::{Deserialize, Serialize};

use super::track::Track;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub bpm: f64,
    pub sample_rate: f64,
    pub lpb: u16,
    pub nlines: usize,
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            bpm: 128.0,
            sample_rate: 48000.0,
            lpb: 4,
            nlines: 16,
            tracks: vec![],
        }
    }

    pub fn add_track(&mut self) {
        let mut track = Track::new();
        track.name = format!("T{:02X}", self.tracks.len() + 1);
        self.tracks.push(track);
    }
}
