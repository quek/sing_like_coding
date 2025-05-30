use std::{ops::Range, pin::Pin};

use crate::{
    audio_buffer::AudioBuffer,
    event_list::{EventListInput, EventListOutput},
};

pub struct ProcessContext {
    pub steady_time: i64,
    pub play_p: bool,
    pub play_position: Range<i64>,
    pub nchannels: usize,
    pub nframes: usize,
    pub buffers: Vec<AudioBuffer>,
    pub event_list_inputs: Vec<Pin<Box<EventListInput>>>,
    pub event_list_outputs: Vec<Pin<Box<EventListOutput>>>,
}

impl Default for ProcessContext {
    fn default() -> Self {
        Self {
            steady_time: 0,
            play_p: false,
            play_position: 0..0,
            nchannels: 2,
            nframes: 512,
            buffers: vec![],
            event_list_inputs: vec![],
            event_list_outputs: vec![],
        }
    }
}

impl ProcessContext {
    pub fn add_track(&mut self) {
        self.buffers.push(AudioBuffer::new());
        self.event_list_inputs.push(EventListInput::new());
        self.event_list_outputs.push(EventListOutput::new());
    }

    pub fn clear_event_lists(&mut self) {
        for event_list in self.event_list_inputs.iter_mut() {
            event_list.clear();
        }
        for event_list in self.event_list_outputs.iter_mut() {
            event_list.clear();
        }
    }

    pub fn ensure_buffer(&mut self) {
        for buffer in self.buffers.iter_mut() {
            buffer.ensure_buffer(self.nchannels, self.nframes);
        }
    }
}
