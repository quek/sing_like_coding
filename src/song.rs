use std::{
    ops::Range,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use crate::{
    note::Note,
    track::{self, Track},
    track_view::ViewCommand,
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum SongCommand {
    Track,
    Note,
    StateTrack(String, usize, Vec<Note>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub bpm: f64,
    pub sample_rate: f64,
    pub lpb: u16,
    pub play_p: bool,
    pub play_position: Range<i64>,
    pub tracks: Vec<track::State>,
}

impl State {
    pub fn new() -> Self {
        Self {
            bpm: 128.0,
            sample_rate: 48000.0,
            lpb: 4,
            play_p: false,
            play_position: (0..0),
            tracks: vec![],
        }
    }
}

pub struct Song {
    pub state: State,
    pub tracks: Vec<Track>,
    song_sender: Sender<SongCommand>,
}

unsafe impl Send for Song {}
unsafe impl Sync for Song {}

impl Song {
    pub fn new(song_sender: Sender<SongCommand>) -> Self {
        Self {
            state: State::new(),
            tracks: vec![Track::new()],
            song_sender,
        }
    }

    fn compute_play_position(&mut self, frames_count: u32) {
        self.state.play_position.start = self.state.play_position.end;
        if !self.state.play_p {
            return;
        }
        let sec_per_frame = frames_count as f64 / self.state.sample_rate;
        let sec_per_delay = 60.0 / (self.state.bpm * self.state.lpb as f64 * 256.0);
        self.state.play_position.end =
            self.state.play_position.start + (sec_per_frame / sec_per_delay).round() as i64;
    }

    pub fn process(
        &mut self,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        self.compute_play_position(frames_count);

        let track = &mut self.tracks[0];
        track.process(&self.state, buffer, frames_count, steady_time)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn play(&mut self) {
        if self.state.play_p {
            return;
        }
        self.state.play_p = true;
    }

    pub fn start_listener(song: Arc<Mutex<Self>>, receiver: Receiver<ViewCommand>) {
        log::debug!("Song::start_listener");
        thread::spawn(move || {
            while let Ok(msg) = receiver.recv() {
                log::debug!("Song 受信 {:?}", msg);
                // メッセージに応じた処理をここに書く
                match msg {
                    ViewCommand::Play => song.lock().unwrap().play(),
                    ViewCommand::Stop => song.lock().unwrap().stop(),
                    ViewCommand::StateTrack(index) => {
                        song.lock().unwrap().send_state_track(index);
                    }
                    ViewCommand::Note(line, key) => {
                        log::debug!("ViewCommand::Note({line}, {key})");
                        let mut song = song.lock().unwrap();
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
                        song.send_state_track(0);
                    }
                }
            }
        });
    }

    pub fn send_state_track(&self, index: usize) {
        let track = &self.tracks[index];
        let notes = track.notes.clone();
        self.song_sender
            .send(SongCommand::StateTrack(
                track.state.name.clone(),
                track.state.nlines,
                notes,
            ))
            .unwrap();
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) {
        if !self.state.play_p {
            return;
        }
        self.state.play_p = false;
    }
}
