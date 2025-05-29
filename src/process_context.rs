use std::ops::Range;

pub struct ProcessContext {
    pub bpm: f64,
    pub sample_rate: f64,
    pub steady_time: i64,
    pub lpb: u16,
    pub play_p: bool,
    pub play_position: Range<i64>,
    pub channels: usize,
    pub nframes: u32,
    pub buffer: Vec<Vec<f32>>,
}
