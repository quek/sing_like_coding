use std::{
    path::Path,
    pin::Pin,
    sync::{mpsc::Sender, Arc, Mutex},
};

use clap_sys::plugin::clap_plugin;

use crate::{plugin::Plugin, song::Song};

pub struct AudioProcess {
    plugin: Option<Pin<Box<Plugin>>>,
    steady_time: i64,
    buffer: Vec<Vec<f32>>,
    _callback_request_sender: Sender<*const clap_plugin>,
    gui_context: Option<eframe::egui::Context>,
    song: Arc<Mutex<Song>>,
}

unsafe impl Send for AudioProcess {}
unsafe impl Sync for AudioProcess {}

impl AudioProcess {
    pub fn new(
        callback_request_sender: Sender<*const clap_plugin>,
        song: Arc<Mutex<Song>>,
    ) -> Self {
        let mut plugin = Plugin::new(callback_request_sender.clone());
        let path = Path::new("c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap");
        //let path = Path::new("c:/Program Files/Common Files/CLAP/VCV Rack 2.clap");
        //let path = Path::new("c:/Program Files/Common Files/CLAP/kern64.clap");
        plugin.load(path);
        plugin.start().unwrap();
        plugin.gui_open().unwrap();

        Self {
            plugin: Some(plugin),
            steady_time: 0,
            buffer: vec![vec![0.0; 256], vec![0.0; 256]],
            _callback_request_sender: callback_request_sender,
            gui_context: None,
            song,
        }
    }

    pub fn process(&mut self, output: &mut [f32], channels: usize) {
        // TODO
        // TODO
        // TODO
        // TODO
        self.song.lock().unwrap().process().unwrap();
        // TODO
        // TODO
        // TODO
        // TODO

        //log::debug!("AudioProcess process steady_time {}", self.steady_time);
        let frames_count = output.len() / channels;
        if self.buffer.len() < channels || self.buffer[0].len() < frames_count {
            //log::debug!("realloc AudioProcess buffer {}", frames_count);
            self.buffer.clear();
            for _ in 0..channels {
                self.buffer.push(vec![0.0; frames_count]);
            }
        }
        self.plugin
            .as_mut()
            .unwrap()
            .process(&mut self.buffer, frames_count as u32, self.steady_time)
            .unwrap();

        for channel in 0..channels {
            for frame in 0..frames_count {
                output[channels * frame + channel] = self.buffer[channel][frame];
            }
        }
        self.steady_time += frames_count as i64;
    }

    pub fn set_gui_context(&mut self, context: &eframe::egui::Context) {
        self.gui_context = Some(context.clone());
        self.plugin
            .as_mut()
            .map(|x| x.gui_context = Some(context.clone()));
    }
}
