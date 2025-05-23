use std::path::Path;

use crate::plugin::Plugin;

pub struct AudioProcess {
    plugin: Option<Plugin>,
    steady_time: i64,
}

unsafe impl Send for AudioProcess {}
unsafe impl Sync for AudioProcess {}

impl AudioProcess {
    pub fn new() -> Self {
        let mut plugin = Plugin::new();
        let path = Path::new("c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap");
        //let path = Path::new("c:/Program Files/Common Files/CLAP/VCV Rack 2.clap");
        plugin.load(path);
        plugin.start().unwrap();
        plugin.gui_open().unwrap();

        Self {
            plugin: Some(plugin),
            steady_time: 0,
        }
    }

    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        let frames_count = output.len() / channels;
        let buffer = self
            .plugin
            .as_mut()
            .unwrap()
            .process(frames_count as u32, self.steady_time)
            .unwrap();

        for (i, frame) in output.chunks_mut(channels).enumerate() {
            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = buffer[channel][i];
            }
        }
        self.steady_time += frames_count as i64;
    }
}
