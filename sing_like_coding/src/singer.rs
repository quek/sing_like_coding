use std::{
    ops::Range,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
};

use crate::{
    clap_manager::Description,
    model::{note::Note, song::Song},
    util::next_id,
    view::main_view::ViewMsg,
};

use anyhow::Result;
use clap_sys::plugin::clap_plugin;
use common::{
    event::Event, module::Module, plugin_ref::PluginRef,
    process_track_context::ProcessTrackContext, protocol::MainToPlugin,
};
use futures::future::{join_all, try_join_all};
use rayon::prelude::*;
use tokio::{net::windows::named_pipe::ServerOptions, task};

#[derive(Debug)]
pub struct ClapPluginPtr(pub *const clap_plugin);
unsafe impl Send for ClapPluginPtr {}
unsafe impl Sync for ClapPluginPtr {}

#[derive(Debug)]
pub enum SingerMsg {
    Play,
    Stop,
    Song,
    Note(usize, usize, i16),
    #[allow(dead_code)]
    NoteOn(usize, i16, i16, f64, u32),
    #[allow(dead_code)]
    NoteOff(usize, i16, i16, f64, u32),
    PluginLoad(usize, Description),
    TrackAdd,
}

#[derive(Debug, Default)]
pub struct SongState {
    pub play_p: bool,
    pub line_play: usize,
}

pub struct Singer {
    pub steady_time: i64,
    pub play_p: bool,
    pub play_position: Range<i64>,
    pub song: Song,
    song_sender: Sender<ViewMsg>,
    pub sender_to_loop: Sender<MainToPlugin>,
    line_play: usize,
    process_track_contexts: Vec<Arc<Mutex<ProcessTrackContext>>>,
}

unsafe impl Send for Singer {}
unsafe impl Sync for Singer {}

impl Singer {
    pub fn new(song_sender: Sender<ViewMsg>, sender_to_loop: Sender<MainToPlugin>) -> Self {
        let song = Song::new();
        let mut this = Self {
            steady_time: 0,
            play_p: false,
            play_position: (0..0),
            song,
            song_sender,
            sender_to_loop,
            line_play: 0,
            process_track_contexts: vec![],
        };
        this.add_track();
        this
    }

    fn add_track(&mut self) {
        self.song.add_track();
        self.process_track_contexts
            .push(Arc::new(Mutex::new(ProcessTrackContext::default())));
    }

    fn compute_play_position(&mut self, frames_count: usize) {
        self.play_position.start = self.play_position.end;

        let line = (self.play_position.start / 0x100) as usize;
        if self.line_play != line {
            self.send_state();
        }
        self.line_play = line;

        if !self.play_p {
            return;
        }
        let sec_per_frame = frames_count as f64 / self.song.sample_rate;
        let sec_per_delay = 60.0 / (self.song.bpm * self.song.lpb as f64 * 256.0);
        self.play_position.end =
            self.play_position.start + (sec_per_frame / sec_per_delay).round() as i64;

        // TODO DELET THIS BLOC
        {
            if self.play_position.start > 0x0e * 0x100 {
                self.play_position = 0..0;
            }
        }
    }

    pub fn process(&mut self, output: &mut [f32], nchannels: usize) -> Result<()> {
        //log::debug!("AudioProcess process steady_time {}", self.steady_time);
        let nframes = output.len() / nchannels;

        self.compute_play_position(nframes);

        for context in self.process_track_contexts.iter_mut() {
            let mut context = context.lock().unwrap();
            context.nchannels = nchannels;
            context.nframes = nframes;
            context.play_p = self.play_p;
            context.bpm = self.song.bpm;
            context.steady_time = self.steady_time;
            context.play_position = self.play_position.clone();
            context.prepare();
        }

        // let _ = self
        //     .song
        //     .tracks
        //     .par_iter()
        //     .zip(self.process_track_contexts.par_iter_mut())
        //     .try_for_each(|(track, process_track_context)| track.process(process_track_context));

        let contexts: Vec<_> = self
            .process_track_contexts
            .iter()
            .map(|x| x.clone())
            .collect();
        let futures = self
            .song
            .tracks
            .iter()
            .zip(contexts.into_iter())
            .map(|(track, ctx)| {
                let track = track.clone();
                async move {
                    let ctx = ctx.clone();
                    // log::debug!("#### before track.process");
                    let r = track.process(ctx).await;
                    // log::debug!("#### after track.process");
                    r
                }
            });

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let _ = runtime.block_on(async {
            //log::debug!("#### before join_all");
            let _ = join_all(futures).await;
            //log::debug!("#### after join_all");
            Ok::<(), anyhow::Error>(())
        });

        for channel in 0..nchannels {
            for frame in 0..nframes {
                output[nchannels * frame + channel] = self
                    .process_track_contexts
                    .iter()
                    .map(|x| x.lock().unwrap().buffer.buffer[channel][frame])
                    .sum();
            }
        }

        self.steady_time += nframes as i64;

        Ok(())
    }

    pub fn play(&mut self) {
        if self.play_p {
            return;
        }
        self.play_p = true;
    }

    pub fn stop(&mut self) {
        if !self.play_p {
            return;
        }
        self.play_p = false;
    }

    pub fn start_listener(singer: Arc<Mutex<Self>>, receiver: Receiver<SingerMsg>) {
        log::debug!("Song::start_listener");
        tokio::spawn(async move {
            singer_loop(singer, receiver).await.unwrap();
        });
    }

    fn send_song(&self) {
        self.song_sender
            .send(ViewMsg::Song(self.song.clone()))
            .unwrap();
    }

    fn send_state(&self) {
        self.song_sender
            .send(ViewMsg::State(SongState {
                play_p: self.play_p,
                line_play: self.line_play,
            }))
            .unwrap();
    }
}

async fn singer_loop(
    singer: Arc<Mutex<Singer>>,
    receiver: Receiver<SingerMsg>,
) -> anyhow::Result<()> {
    while let Ok(msg) = receiver.recv() {
        log::debug!("Song 受信 {:?}", msg);
        match msg {
            SingerMsg::Play => {
                let mut singer = singer.lock().unwrap();
                singer.play();
                singer.send_state();
            }
            SingerMsg::Stop => {
                let mut singer = singer.lock().unwrap();
                singer.stop();
                singer.send_state();
            }
            SingerMsg::Song => singer.lock().unwrap().send_song(),
            SingerMsg::Note(track_index, line, key) => {
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
            SingerMsg::PluginLoad(track_index, description) => {
                log::debug!("will send MainToPlugin::Load {:?}", description);
                let pipe_name = format!(r"\\.\pipe\sing_like_coding\plugin\{}", next_id());
                let pipe = ServerOptions::new().create(&pipe_name)?;

                {
                    let singer = singer.lock().unwrap();
                    singer.sender_to_loop.send(MainToPlugin::Load(
                        description.id.clone(),
                        pipe_name,
                        track_index,
                    ))?;
                }

                pipe.connect().await?;

                {
                    let mut singer = singer.lock().unwrap();
                    singer.song.tracks[track_index].modules.push(Module::new(
                        description.id.clone(),
                        description.name.clone(),
                    ));
                    singer.process_track_contexts[track_index]
                        .lock()
                        .unwrap()
                        .plugins
                        .push(PluginRef::new(pipe));
                    singer.send_song();
                }
            }
            SingerMsg::NoteOn(track_index, key, _channel, velocity, _time) => {
                let singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .lock()
                    .unwrap()
                    .event_list_input
                    .push(Event::NoteOn(key, velocity));
            }
            SingerMsg::NoteOff(track_index, key, _channel, _velocity, _time) => {
                let singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .lock()
                    .unwrap()
                    .event_list_input
                    .push(Event::NoteOff(key));
            }
            SingerMsg::TrackAdd => {
                let mut singer = singer.lock().unwrap();
                singer.add_track();
                singer.send_song();
            }
        }
    }
    Ok(())
}
