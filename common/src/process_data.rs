use clap_sys::id::clap_id;

use crate::dsp::linear_to_db;

pub const MAX_CHANNELS: usize = 2;
pub const MAX_FRAMES: usize = 2048;
pub const MAX_EVENTS: usize = 128;
const MAX_PORTS: usize = 8;

#[repr(C)]
pub struct ProcessData {
    pub nchannels: usize,
    pub nframes: usize,
    pub play_p: u8,
    pub bpm: f64,
    pub lpb: u16,
    pub sample_rate: f64,
    pub steady_time: i64,
    pub nevents_input: usize,
    pub events_input: [Event; MAX_EVENTS],
    pub nevents_output: usize,
    pub events_output: [Event; MAX_EVENTS],
    pub nports_in: usize,
    pub buffer_in: [[[f32; MAX_FRAMES]; MAX_CHANNELS]; MAX_PORTS],
    pub nports_out: usize,
    pub buffer_out: [[[f32; MAX_FRAMES]; MAX_CHANNELS]; MAX_PORTS],
    pub constant_mask_in: [u64; MAX_PORTS],
    pub constant_mask_out: [u64; MAX_PORTS],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Event {
    pub kind: EventKind,
    pub key: i16,
    pub velocity: f64,
    pub channel: i16,
    pub param_id: clap_id,
    pub value: f64,
    pub delay: usize,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum EventKind {
    NoteOn = 1,
    NoteOff = 2,
    ParamValue = 3,
}

impl ProcessData {
    pub fn new() -> Self {
        Self {
            nchannels: MAX_CHANNELS,
            nframes: MAX_FRAMES,
            play_p: 0,
            bpm: 120.0,
            lpb: 4,
            sample_rate: 48000.0,
            steady_time: 0,
            nevents_input: 0,
            events_input: [Event {
                kind: EventKind::NoteOn,
                key: 0,
                velocity: 0.0,
                channel: 0,
                param_id: 0,
                value: 0.0,
                delay: 0,
            }; MAX_EVENTS],
            nevents_output: 0,
            events_output: [Event {
                kind: EventKind::NoteOn,
                key: 0,
                velocity: 0.0,
                channel: 0,
                param_id: 0,
                value: 0.0,
                delay: 0,
            }; MAX_EVENTS],
            nports_in: 1,
            buffer_in: [[[0.0; MAX_FRAMES]; MAX_CHANNELS]; MAX_PORTS],
            nports_out: 1,
            buffer_out: [[[0.0; MAX_FRAMES]; MAX_CHANNELS]; MAX_PORTS],
            constant_mask_in: [0; MAX_PORTS],
            constant_mask_out: [0; MAX_PORTS],
        }
    }

    pub fn peak(&self, port: usize, channel: usize) -> f32 {
        let value = if self.constant_mask_out[port] & (1 << channel) == 0 {
            self.buffer_out[port][channel][..self.nframes]
                .iter()
                .fold(0.0, |acc: f32, x| acc.max(x.abs()))
        } else {
            self.buffer_out[port][channel][0].abs()
        };
        linear_to_db(value)
    }

    pub fn prepare(&mut self) {
        self.nevents_input = 0;
        self.nevents_output = 0;
        for port in 0..MAX_PORTS {
            for channel in 0..MAX_CHANNELS {
                self.buffer_in[port][channel][0] = 0.0;
                self.buffer_out[port][channel][0] = 0.0;
                let bit = 1 << channel;
                self.constant_mask_in[port] |= bit;
                self.constant_mask_out[port] |= bit;
            }
        }
    }

    pub fn input_note_on(&mut self, key: i16, velocity: f64, channel: i16, delay: usize) {
        if self.nevents_input == MAX_EVENTS {
            panic!();
        }
        self.events_input[self.nevents_input].kind = EventKind::NoteOn;
        self.events_input[self.nevents_input].key = key;
        self.events_input[self.nevents_input].velocity = velocity;
        self.events_input[self.nevents_input].channel = channel;
        self.events_input[self.nevents_input].delay = delay;
        self.nevents_input += 1;
    }

    pub fn input_note_off(&mut self, key: i16, channel: i16, delay: usize) {
        if self.nevents_input == MAX_EVENTS {
            panic!();
        }
        self.events_input[self.nevents_input].kind = EventKind::NoteOff;
        self.events_input[self.nevents_input].key = key;
        self.events_input[self.nevents_input].velocity = 0.0;
        self.events_input[self.nevents_input].channel = channel;
        self.events_input[self.nevents_input].delay = delay;
        self.nevents_input += 1;
    }

    pub fn input_param_value(&mut self, param_id: clap_id, value: f64, delay: usize) {
        if self.nevents_input == MAX_EVENTS {
            panic!();
        }
        self.events_input[self.nevents_input].kind = EventKind::ParamValue;
        self.events_input[self.nevents_input].param_id = param_id;
        self.events_input[self.nevents_input].value = value;
        self.events_input[self.nevents_input].delay = delay;
        self.nevents_input += 1;
    }

    pub fn output_note_on(&mut self, key: i16, velocity: f64, channel: i16, delay: usize) {
        if self.nevents_output == MAX_EVENTS {
            panic!();
        }
        self.events_output[self.nevents_output].kind = EventKind::NoteOn;
        self.events_output[self.nevents_output].key = key;
        self.events_output[self.nevents_output].velocity = velocity;
        self.events_output[self.nevents_output].channel = channel;
        self.events_output[self.nevents_output].delay = delay;
        self.nevents_output += 1;
    }

    pub fn output_note_off(&mut self, key: i16, channel: i16, delay: usize) {
        if self.nevents_output == MAX_EVENTS {
            panic!();
        }
        self.events_output[self.nevents_output].kind = EventKind::NoteOff;
        self.events_output[self.nevents_output].key = key;
        self.events_output[self.nevents_output].velocity = 0.0;
        self.events_output[self.nevents_output].channel = channel;
        self.events_output[self.nevents_output].delay = delay;
        self.nevents_output += 1;
    }

    pub fn output_param_value(&mut self, param_id: clap_id, value: f64, delay: usize) {
        if self.nevents_output == MAX_EVENTS {
            panic!();
        }
        self.events_output[self.nevents_output].kind = EventKind::ParamValue;
        self.events_output[self.nevents_output].param_id = param_id;
        self.events_output[self.nevents_output].value = value;
        self.events_output[self.nevents_output].delay = delay;
        self.nevents_output += 1;
    }
}
