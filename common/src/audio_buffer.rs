use bincode::{Decode, Encode};

#[repr(C)]
#[derive(Clone, Default, Encode, Decode, PartialEq, Debug)]
pub struct AudioBuffer {
    pub buffer: Vec<Vec<f32>>,
    pub constant_mask: u64,
}

impl AudioBuffer {
    pub fn zero(&mut self) {
        for (i, buffer) in self.buffer.iter_mut().enumerate() {
            buffer[0] = 0.0;
            self.constant_mask |= 1 << i;
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
