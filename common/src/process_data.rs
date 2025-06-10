use crate::dsp::linear_to_db;

pub const MAX_CHANNELS: usize = 2;
pub const MAX_FRAMES: usize = 2048;
pub const MAX_EVENTS: usize = 64;

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
    pub buffer_in: [[f32; MAX_FRAMES]; MAX_CHANNELS],
    pub buffer_out: [[f32; MAX_FRAMES]; MAX_CHANNELS],
    pub constant_mask_in: u64,
    pub constant_mask_out: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub key: i16,
    pub velocity: f64,
    pub channel: i16,
    pub delay: u8,
}

#[repr(u8)]
#[derive(Copy, Clone)]
pub enum EventKind {
    NoteOn = 1,
    NoteOff = 2,
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
                delay: 0,
            }; MAX_EVENTS],
            buffer_in: [[0.0; MAX_FRAMES]; MAX_CHANNELS],
            buffer_out: [[0.0; MAX_FRAMES]; MAX_CHANNELS],
            constant_mask_in: 0,
            constant_mask_out: 0,
        }
    }

    pub fn peak(&self, channel: usize) -> f32 {
        let value = if self.constant_mask_out & (1 << channel) == 0 {
            self.buffer_out[channel][..self.nframes]
                .iter()
                .fold(0.0, |acc: f32, x| acc.max(x.abs()))
        } else {
            self.buffer_out[channel][0].abs()
        };
        linear_to_db(value)
    }

    pub fn prepare(&mut self) {
        self.nevents_input = 0;
        for channel in 0..MAX_CHANNELS {
            self.buffer_in[channel][0] = 0.0;
            self.buffer_out[channel][0] = 0.0;
            self.constant_mask_in |= 1 << channel;
            self.constant_mask_out |= 1 << channel;
        }
    }

    pub fn note_on(&mut self, key: i16, velocity: f64, channel: i16, delay: u8) {
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

    pub fn note_off(&mut self, key: i16, channel: i16, delay: u8) {
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
}
