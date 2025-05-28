use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use crate::{note::Note, track::Track, track_view::ViewCommand};

use anyhow::Result;

#[derive(Debug)]
pub enum SongCommand {
    Track,
    Note,
    StateTrack(String, usize, Vec<Note>),
}

pub struct Song {
    _bpm: f32,
    _lpb: u16,
    play_p: bool,
    _play_position: i64,
    pub tracks: Vec<Track>,
    song_sender: Sender<SongCommand>,
}

unsafe impl Send for Song {}
unsafe impl Sync for Song {}

impl Song {
    pub fn new(song_sender: Sender<SongCommand>) -> Self {
        Self {
            _bpm: 128.0,
            _lpb: 4,
            play_p: false,
            _play_position: 0,
            tracks: vec![Track::new()],
            song_sender,
        }
    }

    pub fn process(
        &mut self,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        self.tracks[0].process(buffer, frames_count, steady_time)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn play(&mut self) {
        if self.play_p {
            return;
        }
        self.play_p = true;
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
                        song.lock().unwrap().state_track(index);
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
                        song.state_track(0);
                    }
                }
            }
        });
    }

    pub fn state_track(&self, index: usize) {
        let track = &self.tracks[index];
        let notes = track.notes.clone();
        self.song_sender
            .send(SongCommand::StateTrack(
                track.name.clone(),
                track.nlines,
                notes,
            ))
            .unwrap();
    }

    #[allow(dead_code)]
    pub fn stop(&mut self) {
        if !self.play_p {
            return;
        }
        self.play_p = false;
    }
}
