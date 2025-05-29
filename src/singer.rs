use std::{
    path::Path,
    pin::Pin,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use crate::{
    event_list::{EventListInput, EventListOutput},
    model::{self, note::Note, Song},
    plugin::Plugin,
    track_view::ViewCommand,
};

use anyhow::Result;
use clap_sys::plugin::clap_plugin;

#[derive(Debug)]
pub struct ClapPluginPtr(pub *const clap_plugin);
unsafe impl Send for ClapPluginPtr {}
unsafe impl Sync for ClapPluginPtr {}

#[derive(Debug)]
pub enum SongCommand {
    #[allow(dead_code)]
    Track,
    Song(Song),
    State(SongState),
    PluginCallback(ClapPluginPtr),
}

#[derive(Debug, Default)]
pub struct SongState {
    pub line_play: usize,
}

pub struct Singer {
    pub song: model::Song,
    song_sender: Sender<SongCommand>,
    pub plugins: Vec<Vec<Pin<Box<Plugin>>>>,
    event_list_inputs: Vec<Pin<Box<EventListInput>>>,
    event_list_outputs: Vec<Pin<Box<EventListOutput>>>,
    pub gui_context: Option<eframe::egui::Context>,
    line_play: usize,
    on_keys: Vec<Option<i16>>,
}

unsafe impl Send for Singer {}
unsafe impl Sync for Singer {}

impl Singer {
    pub fn new(song_sender: Sender<SongCommand>) -> Self {
        let song = model::Song::new();
        let mut this = Self {
            song,
            song_sender,
            plugins: Default::default(),
            event_list_inputs: vec![],
            event_list_outputs: vec![],
            gui_context: None,
            line_play: 0,
            on_keys: vec![],
        };
        this.add_track();
        this
    }

    fn add_track(&mut self) {
        self.song.add_track();
        self.event_list_inputs.push(EventListInput::new());
        self.event_list_outputs.push(EventListOutput::new());
        self.on_keys.push(None);
    }

    fn compute_play_position(&mut self, frames_count: u32) {
        self.song.play_position.start = self.song.play_position.end;

        let line = (self.song.play_position.start / 0x100) as usize;
        if self.line_play != line {
            self.song_sender
                .send(SongCommand::State(SongState {
                    line_play: self.line_play,
                }))
                .unwrap();
        }
        self.line_play = line;

        if !self.song.play_p {
            return;
        }
        let sec_per_frame = frames_count as f64 / self.song.sample_rate;
        let sec_per_delay = 60.0 / (self.song.bpm * self.song.lpb as f64 * 256.0);
        self.song.play_position.end =
            self.song.play_position.start + (sec_per_frame / sec_per_delay).round() as i64;

        // TODO DELET THIS BLOC
        {
            if self.song.play_position.start > 0x0e * 0x100 {
                self.song.play_position = 0..0;
            }
        }
    }

    pub fn process(
        &mut self,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        //log::debug!("start singer process");
        self.compute_play_position(frames_count);

        self.process_track(0, buffer, frames_count, steady_time)?;

        for x in self.event_list_inputs.iter_mut() {
            x.clear();
        }
        for x in self.event_list_outputs.iter_mut() {
            x.clear();
        }

        Ok(())
    }

    fn process_track(
        &mut self,
        track_index: usize,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        let track = &self.song.tracks[track_index];
        let on_keys = &mut self.on_keys[track_index];
        track.compute_midi(
            &self.song.play_position,
            &mut self.event_list_inputs[track_index],
            on_keys,
        );
        let module_len = self.song.tracks[track_index].modules.len();
        for i in 0..module_len {
            self.process_module(track_index, i, buffer, frames_count, steady_time)?;
        }
        Ok(())
    }

    fn process_module(
        &mut self,
        track_index: usize,
        module_index: usize,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        let plugin = &mut self.plugins[track_index][module_index];
        let event_list_input = &mut self.event_list_inputs[track_index];
        let event_list_output = &mut self.event_list_outputs[track_index];
        plugin.process(
            &self.song,
            buffer,
            frames_count,
            steady_time,
            event_list_input,
            event_list_output,
        )?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn play(&mut self) {
        if self.song.play_p {
            return;
        }
        self.song.play_p = true;
    }

    pub fn start_listener(singer: Arc<Mutex<Self>>, receiver: Receiver<ViewCommand>) {
        log::debug!("Song::start_listener");
        thread::spawn(move || {
            while let Ok(msg) = receiver.recv() {
                log::debug!("Song 受信 {:?}", msg);
                match msg {
                    ViewCommand::Play => singer.lock().unwrap().play(),
                    ViewCommand::Stop => singer.lock().unwrap().stop(),
                    ViewCommand::Song => singer.lock().unwrap().send_sond(),
                    ViewCommand::Note(line, key) => {
                        log::debug!("ViewCommand::Note({line}, {key})");
                        let mut singer = singer.lock().unwrap();
                        let song = &mut singer.song;
                        let track = &mut song.tracks[0];
                        if let Some(note) = track.note_mut(line) {
                            note.key = key;
                        } else {
                            track.notes.push(Note {
                                line,
                                delay: 0,
                                channel: 0,
                                key,
                                velocity: 100.0,
                            });
                        }
                        singer.send_sond();
                    }
                    ViewCommand::PluginLoad(track_index, path) => {
                        let mut singer = singer.lock().unwrap();
                        let mut plugin = Plugin::new(singer.song_sender.clone());
                        plugin.load(Path::new(&path));
                        plugin.start().unwrap();
                        singer.song.tracks[track_index]
                            .modules
                            .push(model::Module::new(path));
                        loop {
                            if singer.plugins.len() > track_index {
                                break;
                            }
                            singer.plugins.push(vec![]);
                        }
                        singer.plugins[track_index].push(plugin);
                    }
                    ViewCommand::NoteOn(track_index, key, channel, velocity, time) => {
                        let mut singer = singer.lock().unwrap();
                        singer.event_list_inputs[track_index].note_on(key, channel, velocity, time);
                    }
                    ViewCommand::NoteOff(track_index, key, channel, velocity, time) => {
                        let mut singer = singer.lock().unwrap();
                        singer.event_list_inputs[track_index]
                            .note_off(key, channel, velocity, time);
                    }
                }
            }
        });
    }

    fn send_sond(&self) {
        self.song_sender
            .send(SongCommand::Song(self.song.clone()))
            .unwrap();
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) {
        if !self.song.play_p {
            return;
        }
        self.song.play_p = false;
    }
}
