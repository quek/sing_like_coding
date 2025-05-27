use crate::track::Track;

use anyhow::Result;

pub struct Song {
    bpm: f32,
    lpb: u16,
    play_p: bool,
    play_position: i64,
    pub tracks: Vec<Track>,
}

impl Song {
    pub fn new() -> Self {
        Self {
            bpm: 128.0,
            lpb: 4,
            play_p: false,
            play_position: 0,
            tracks: vec![Track::new()],
        }
    }

    pub fn process(
        &mut self,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        self.tracks[0].process(buffer, frames_count, steady_time)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn start(&mut self) {
        if self.play_p {
            return;
        }
        self.play_p = true;
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) {
        if !self.play_p {
            return;
        }
        self.play_p = false;
    }
}
