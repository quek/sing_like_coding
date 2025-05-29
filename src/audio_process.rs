use std::sync::{Arc, Mutex};

use crate::singer::Singer;

pub struct AudioProcess {
    steady_time: i64,
    buffer: Vec<Vec<f32>>,
    singer: Arc<Mutex<Singer>>,
}

unsafe impl Send for AudioProcess {}
unsafe impl Sync for AudioProcess {}

impl AudioProcess {
    pub fn new(song: Arc<Mutex<Singer>>) -> Self {
        Self {
            steady_time: 0,
            buffer: vec![vec![0.0; 256], vec![0.0; 256]],
            singer: song,
        }
    }

    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        //log::debug!("AudioProcess process steady_time {}", self.steady_time);
        let frames_count = output.len() / channels;
        if self.buffer.len() < channels || self.buffer[0].len() < frames_count {
            //log::debug!("realloc AudioProcess buffer {}", frames_count);
            self.buffer.clear();
            for _ in 0..channels {
                self.buffer.push(vec![0.0; frames_count]);
            }
        }

        //log::debug!("will singer lock process");
        self.singer
            .lock()
            .unwrap()
            .process(&mut self.buffer, frames_count as u32, self.steady_time)
            .unwrap();
        //log::debug!("did singer lock process");

        for channel in 0..channels {
            for frame in 0..frames_count {
                output[channels * frame + channel] = self.buffer[channel][frame];
            }
        }
        self.steady_time += frames_count as i64;
    }
}
