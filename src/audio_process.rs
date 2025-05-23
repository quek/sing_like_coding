use std::path::Path;

use crate::plugin::Plugin;

pub struct AudioProcess {
    plugin: Option<Plugin>,
}

unsafe impl Send for AudioProcess {}
unsafe impl Sync for AudioProcess {}

impl AudioProcess {
    pub fn new() -> Self {
        let mut plugin = Plugin::new();
        let path = Path::new("c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap");
        plugin.load(path);
        plugin.start().unwrap();
        plugin.gui_open().unwrap();

        Self {
            plugin: Some(plugin),
        }
    }

    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        let frames_count = output.len() / channels;
        let buffer = self
            .plugin
            .as_mut()
            .unwrap()
            .process(frames_count as u32)
            .unwrap();

        for (i, frame) in output.chunks_mut(channels).enumerate() {
            for (channel, sample) in frame.iter_mut().enumerate() {
                *sample = buffer[channel][i];
            }
        }
    }
}
