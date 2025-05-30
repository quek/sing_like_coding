use std::ops::Range;

use crate::audio_buffer::AudioBuffer;

pub enum Event {
    NoteOn(i16, f64),
    NoteOff(i16),
}

pub struct ProcessContext {
    pub steady_time: i64,
    pub play_p: bool,
    pub play_position: Range<i64>,
    pub nchannels: usize,

    pub nframes: usize,
    pub buffers: Vec<AudioBuffer>,
    // pub event_list_inputs: Vec<Pin<Box<EventListInput>>>,
    // pub event_list_outputs: Vec<Pin<Box<EventListOutput>>>,
    pub event_list_inputs: Vec<Vec<Event>>,
    pub event_list_outputs: Vec<Vec<Event>>,
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
        self.event_list_inputs.push(vec![]);
        self.event_list_outputs.push(vec![]);
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
