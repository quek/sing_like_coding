use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::app_state::CursorTrack;

use super::{lane_item::LaneItem, track::Track};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub name: String,
    pub bpm: f64,
    pub sample_rate: f64,
    pub lpb: u16,
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            name: Local::now().format("%Y%m%d.json").to_string(),
            bpm: 128.0,
            sample_rate: 48000.0,
            lpb: 4,
            tracks: vec![],
        }
    }

    pub fn track_add(&mut self) {
        let mut track = Track::new();
        track.name = if self.tracks.is_empty() {
            "Main".to_string()
        } else {
            format!("T{:02X}", self.tracks.len())
        };
        self.tracks.push(track);
    }

    pub fn track_delete(&mut self, track_index: usize) {
        self.tracks.remove(track_index);
    }

    pub fn track_insert(&mut self, track_index: usize, track: Track) {
        self.tracks.insert(track_index, track);
    }

    pub fn lane_item(&self, cursor: &CursorTrack) -> Option<&LaneItem> {
        self.tracks
            .get(cursor.track)
            .and_then(|x| x.lanes.get(cursor.lane))
            .and_then(|x| x.item(cursor.line))
    }

    pub fn lane_item_mut(&mut self, cursor: &CursorTrack) -> Option<&mut LaneItem> {
        self.tracks
            .get_mut(cursor.track)
            .and_then(|x| x.lanes.get_mut(cursor.lane))
            .and_then(|x| x.item_mut(cursor.line))
    }
}
