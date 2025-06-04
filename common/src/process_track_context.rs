use std::ops::Range;

use crate::{audio_buffer::AudioBuffer, event::Event};

#[derive(Default)]
pub struct ProcessTrackContext {
    #[allow(dead_code)]
    pub nchannels: usize,
    pub nframes: usize,
    pub buffer: AudioBuffer,
    pub play_p: bool,
    pub bpm: f64,
    pub steady_time: i64,
    pub play_position: Range<i64>,
    pub on_key: Option<i16>,
    pub event_list_input: Vec<Event>,
}

impl ProcessTrackContext {
    pub fn prepare(&mut self) {
        self.event_list_input.clear();
        self.buffer.ensure_buffer(self.nchannels, self.nframes);
    }
}
