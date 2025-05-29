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
    process_context::ProcessContext,
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
    pub gui_context: Option<eframe::egui::Context>,
    line_play: usize,
    on_keys: Vec<Option<i16>>,
    process_context: ProcessContext,
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
            gui_context: None,
            line_play: 0,
            on_keys: vec![],
            process_context: ProcessContext::default(),
        };
        this.add_track();
        this
    }

    fn add_track(&mut self) {
        self.song.add_track();
        self.on_keys.push(None);
        self.process_context
            .event_list_inputs
            .push(EventListInput::new());
        self.process_context
            .event_list_outputs
            .push(EventListOutput::new());
    }

    fn compute_play_position(&mut self, frames_count: usize) {
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

    pub fn process(&mut self, output: &mut [f32], channels: usize) -> Result<()> {
        //log::debug!("AudioProcess process steady_time {}", self.steady_time);
        self.process_context.nframes = output.len() / channels;
        self.process_context.channels = channels;
        self.process_context.ensure_buffer();

        self.compute_play_position(self.process_context.nframes);

        self.process_context.play_p = self.song.play_p;
        self.process_context.play_position = self.song.play_position.clone();

        self.process_context.track_index = 0;
        self.process_track()?;

        for channel in 0..channels {
            for frame in 0..self.process_context.nframes {
                output[channels * frame + channel] = self.process_context.buffer[channel][frame];
            }
        }
        self.process_context.steady_time += self.process_context.nframes as i64;

        Ok(())
    }

    fn process_track(&mut self) -> Result<()> {
        let track_index = self.process_context.track_index;
        let track = &self.song.tracks[track_index];
        let on_keys = &mut self.on_keys[track_index];
        track.compute_midi(&mut self.process_context, on_keys);
        let module_len = self.song.tracks[track_index].modules.len();
        for i in 0..module_len {
            self.process_module(i)?;
        }

        self.process_context.clear_event_lists();

        Ok(())
    }

    fn process_module(&mut self, module_index: usize) -> Result<()> {
        let plugin = &mut self.plugins[self.process_context.track_index][module_index];
        let ctx = &mut self.process_context;
        plugin.process(&self.song, ctx)?;
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
                    ViewCommand::Song => singer.lock().unwrap().send_song(),
                    ViewCommand::Note(track_index, line, key) => {
                        log::debug!("ViewCommand::Note({line}, {key})");
                        let mut singer = singer.lock().unwrap();
                        let song = &mut singer.song;
                        if let Some(track) = song.tracks.get_mut(track_index) {
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
                            singer.send_song();
                        }
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
                        singer.process_context.event_list_inputs[track_index]
                            .note_on(key, channel, velocity, time);
                    }
                    ViewCommand::NoteOff(track_index, key, channel, velocity, time) => {
                        let mut singer = singer.lock().unwrap();
                        singer.process_context.event_list_inputs[track_index]
                            .note_off(key, channel, velocity, time);
                    }
                    ViewCommand::TrackAdd => {
                        let mut singer = singer.lock().unwrap();
                        singer.add_track();
                        singer.send_song();
                    }
                }
            }
        });
    }

    fn send_song(&self) {
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
