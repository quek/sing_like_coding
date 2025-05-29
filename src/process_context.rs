use std::{ops::Range, pin::Pin};

use crate::event_list::{EventListInput, EventListOutput};

pub struct ProcessContext {
    #[allow(dead_code)]
    pub bpm: f64,
    #[allow(dead_code)]
    pub sample_rate: f64,
    pub steady_time: i64,
    #[allow(dead_code)]
    pub lpb: u16,
    pub play_p: bool,
    pub play_position: Range<i64>,
    pub channels: usize,
    pub nframes: usize,
    pub buffer: Vec<Vec<f32>>,
    pub track_index: usize,
    pub event_list_inputs: Vec<Pin<Box<EventListInput>>>,
    pub event_list_outputs: Vec<Pin<Box<EventListOutput>>>,
}

impl Default for ProcessContext {
    fn default() -> Self {
        Self {
            bpm: 120.0,
            sample_rate: 48000.0,
            steady_time: 0,
            lpb: 4,
            play_p: false,
            play_position: 0..0,
            channels: 2,
            nframes: 512,
            buffer: vec![],
            track_index: 0,
            event_list_inputs: vec![],
            event_list_outputs: vec![],
        }
    }
}

impl ProcessContext {
    pub fn clear_event_lists(&mut self) {
        for event_list in self.event_list_inputs.iter_mut() {
            event_list.clear();
        }
        for event_list in self.event_list_outputs.iter_mut() {
            event_list.clear();
        }
    }

    pub fn ensure_buffer(&mut self) {
        if self.buffer.len() < self.channels || self.buffer[0].len() < self.nframes {
            //log::debug!("realloc AudioProcess buffer {}", frames_count);
            self.buffer.clear();
            for _ in 0..self.channels {
                self.buffer.push(vec![0.0; self.nframes]);
            }
        }
    }

    pub fn event_list_input(&mut self) -> &mut EventListInput {
        &mut self.event_list_inputs[self.track_index]
    }

    #[allow(dead_code)]
    pub fn event_list_output(&mut self) -> &mut EventListOutput {
        &mut self.event_list_outputs[self.track_index]
    }
}
