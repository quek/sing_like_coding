use std::ops::Range;

use crate::{audio_buffer::AudioBuffer, event::Event, plugin_ref::PluginRef};

#[derive(Clone, Default)]
pub struct ProcessTrackContext {
    pub nchannels: usize,
    pub nframes: usize,
    pub buffer: AudioBuffer,
    pub play_p: bool,
    pub bpm: f64,
    pub steady_time: i64,
    pub play_position: Range<i64>,
    pub on_key: Option<i16>,
    pub event_list_input: Vec<Event>,
    pub plugins: Vec<PluginRef>,
}

unsafe impl Send for ProcessTrackContext {}
unsafe impl Sync for ProcessTrackContext {}

impl ProcessTrackContext {
    pub fn prepare(&mut self) {
        self.event_list_input.clear();
        self.buffer.ensure_buffer(self.nchannels, self.nframes);
    }
}
