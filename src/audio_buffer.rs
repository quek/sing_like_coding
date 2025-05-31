#[derive(Default)]
pub struct AudioBuffer {
    pub buffer: Vec<Vec<f32>>,
    #[allow(dead_code)]
    pub constant_mask: u64,
}

impl AudioBuffer {
    pub fn new() -> Self {
        Self {
            buffer: vec![],
            constant_mask: 0,
        }
    }

    pub fn ensure_buffer(&mut self, nchannels: usize, nframes: usize) {
        if self.buffer.len() < nchannels || self.buffer[0].len() < nframes {
            self.buffer.clear();
            for _ in 0..nchannels {
                self.buffer.push(vec![0.0; nframes]);
            }
        }
    }
}
