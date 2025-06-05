pub const NCHANNELS_2: usize = 2;
pub const NFRAMES_2048: usize = 2048;
pub const MAX_EVENTS: usize = 64;

#[repr(C)]
pub struct ProcessData {
    pub nchannels: usize,
    pub nframes: usize,
    pub play_p: u8,
    pub bpm: f64,
    pub steady_time: i64,
    pub nevents_input: usize,
    pub events_input: [Event; MAX_EVENTS],
    pub buffer: [[f32; NFRAMES_2048]; NCHANNELS_2],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Event {
    pub kind: EventKind,
    pub key: i16,
    pub velocity: f64,
    pub channel: i16,
    pub time: u32,
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
            nchannels: NCHANNELS_2,
            nframes: NFRAMES_2048,
            play_p: 0,
            bpm: 120.0,
            steady_time: 0,
            nevents_input: 0,
            events_input: [Event {
                kind: EventKind::NoteOn,
                key: 0,
                velocity: 0.0,
                channel: 0,
                time: 0,
            }; MAX_EVENTS],
            buffer: [[0.0; NFRAMES_2048]; NCHANNELS_2],
        }
    }

    pub fn note_on(&mut self, key: i16, velocity: f64, channel: i16, time: u32) {
        if self.nevents_input == MAX_EVENTS {
            panic!();
        }
        self.events_input[self.nevents_input].kind = EventKind::NoteOn;
        self.events_input[self.nevents_input].key = key;
        self.events_input[self.nevents_input].velocity = velocity;
        self.events_input[self.nevents_input].channel = channel;
        self.events_input[self.nevents_input].time = time;
        self.nevents_input += 1;
    }

    pub fn note_off(&mut self, key: i16, channel: i16, time: u32) {
        if self.nevents_input == MAX_EVENTS {
            panic!();
        }
        self.events_input[self.nevents_input].kind = EventKind::NoteOff;
        self.events_input[self.nevents_input].key = key;
        self.events_input[self.nevents_input].velocity = 0.0;
        self.events_input[self.nevents_input].channel = channel;
        self.events_input[self.nevents_input].time = time;
        self.nevents_input += 1;
    }
}
