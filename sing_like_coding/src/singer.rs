use std::{
    fs::File,
    io::BufReader,
    ops::Range,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use crate::{
    app_state::{AppStateCommand, Cursor},
    model::{lane::Lane, note::Note, song::Song},
    util::next_id,
};

use anyhow::Result;
use common::{
    clap_manager::ClapManager, event::Event, module::Module, plugin::description::Description,
    plugin_ref::PluginRef, process_data::ProcessData, process_track_context::ProcessTrackContext,
    protocol::MainToPlugin, shmem::process_data_name,
};
use rayon::prelude::*;
use shared_memory::{Shmem, ShmemConf, ShmemError};

#[derive(Debug)]
pub enum SingerCommand {
    Play,
    Stop,
    Loop,
    Song,
    Note(Cursor, Note),
    NoteDelete(Cursor),
    #[allow(dead_code)]
    NoteOn(usize, i16, i16, f64, u8),
    #[allow(dead_code)]
    NoteOff(usize, i16, i16, f64, u8),
    PluginLoad(usize, Description, isize),
    PluginDelete(usize, usize),
    TrackAdd,
    LaneAdd(usize),
    SongFile(String),
    SongOpen(String, isize),
}

#[derive(Debug, Default)]
pub struct SongState {
    pub song_file: Option<String>,
    pub play_p: bool,
    pub line_play: usize,
    pub loop_p: bool,
    pub loop_range: Range<usize>,
    pub process_elasped_avg: f64,
    pub cpu_usage: f64,
}

pub struct Singer {
    pub song_file: Option<String>,
    pub steady_time: i64,
    pub play_p: bool,
    pub play_position: Range<usize>,
    pub loop_p: bool,
    pub loop_range: Range<usize>,
    all_notef_off_p: bool,
    pub song: Song,
    song_sender: Sender<AppStateCommand>,
    pub sender_to_loop: Sender<MainToPlugin>,
    line_play: usize,
    process_track_contexts: Vec<ProcessTrackContext>,
    shmems: Vec<Vec<Shmem>>,

    cpu_usages: Vec<f64>,
    cpu_usage: f64,
    process_elaspeds: Vec<f64>,
    process_elasped_last: Instant,
    process_elasped_avg: f64,
}

unsafe impl Send for Singer {}
unsafe impl Sync for Singer {}

impl Singer {
    pub fn new(song_sender: Sender<AppStateCommand>, sender_to_loop: Sender<MainToPlugin>) -> Self {
        let song = Song::new();
        let mut this = Self {
            song_file: None,
            steady_time: 0,
            play_p: false,
            play_position: 0..0,
            loop_p: true,
            loop_range: 0..(0x100 * 0x20),
            all_notef_off_p: false,
            song,
            song_sender,
            sender_to_loop,
            line_play: 0,
            process_track_contexts: vec![],
            shmems: vec![],

            cpu_usages: vec![],
            cpu_usage: 0.0,
            process_elaspeds: vec![],
            process_elasped_last: Instant::now(),
            process_elasped_avg: 0.0,
        };
        this.track_add();
        this
    }

    fn compute_play_position(&mut self, frames_count: usize) {
        self.play_position.start = self.play_position.end;

        let line = (self.play_position.start / 0x100) as usize;
        if self.line_play != line {
            self.line_play = line;
            self.send_state();
        } else {
            self.line_play = line;
        }

        if !self.play_p {
            return;
        }

        let sec_per_frame = frames_count as f64 / self.song.sample_rate;
        let sec_per_delay = 60.0 / (self.song.bpm * self.song.lpb as f64 * 256.0);
        self.play_position.end =
            self.play_position.start + (sec_per_frame / sec_per_delay).round() as usize;

        if self.loop_p {
            if self.play_position.end > self.loop_range.end {
                let overflow = self.play_position.end - self.loop_range.end;
                self.play_position.end = self.loop_range.start + overflow;
            }
        }
    }

    pub fn plugin_load(
        &mut self,
        track_index: usize,
        description: &Description,
        gui_open_p: bool,
        hwnd: isize,
    ) -> Result<usize> {
        let id = next_id();

        let shmem_name = process_data_name(id);
        let shmem = ShmemConf::new()
            .size(size_of::<ProcessData>())
            .os_id(dbg!(&shmem_name))
            .create();
        let shmem = match shmem {
            Ok(s) => s,
            Err(ShmemError::MappingIdExists) => ShmemConf::new()
                .os_id(&shmem_name)
                .open()
                .expect("failed to open existing shared memory"),
            Err(e) => panic!("Unexpected shared memory error: {:?}", e),
        };

        self.process_track_contexts[track_index]
            .plugins
            .push(PluginRef::new(id, shmem.as_ptr() as *mut ProcessData)?);
        self.shmems[track_index].push(shmem);

        self.sender_to_loop.send(MainToPlugin::Load(
            id,
            description.id.clone(),
            track_index,
            gui_open_p,
            hwnd,
        ))?;

        self.song.tracks[track_index].modules.push(Module::new(
            description.id.clone(),
            description.name.clone(),
        ));
        Ok(id)
    }

    pub fn process(&mut self, output: &mut [f32], nchannels: usize) -> Result<()> {
        let this_start = Instant::now();

        //log::debug!("AudioProcess process steady_time {}", self.steady_time);
        let nframes = output.len() / nchannels;

        self.compute_play_position(nframes);

        for track_index in 0..self.process_track_contexts.len() {
            for module_index in 0..self.process_track_contexts[track_index].plugins.len() {
                let process_data =
                    self.process_track_contexts[track_index].plugins[module_index].process_data();
                process_data.nchannels = nchannels;
                process_data.nframes = nframes;
                process_data.play_p = if self.play_p { 1 } else { 0 };
                process_data.bpm = self.song.bpm;
                process_data.lpb = self.song.lpb;
                process_data.sample_rate = self.song.sample_rate;
                process_data.steady_time = self.steady_time;
                process_data.prepare();
            }
        }

        for context in self.process_track_contexts.iter_mut() {
            context.nchannels = nchannels;
            context.nframes = nframes;
            context.play_p = self.play_p;
            context.bpm = self.song.bpm;
            context.steady_time = self.steady_time;
            context.play_position = self.play_position.clone();
            context.loop_range = self.loop_range.clone();
            context.prepare();
            if self.all_notef_off_p {
                context.event_list_input.push(Event::NoteAllOff);
            }
        }
        self.all_notef_off_p = false;

        self.song
            .tracks
            .par_iter()
            .zip(self.process_track_contexts.par_iter_mut())
            .try_for_each(|(track, process_track_context)| track.process(process_track_context))?;

        let buffers = self
            .process_track_contexts
            .iter_mut()
            .filter_map(|x| x.plugins.last_mut())
            .map(|plugin_ref: &mut PluginRef| {
                let constant_mask = plugin_ref.process_data().constant_mask_out;
                (&plugin_ref.process_data().buffer_out, constant_mask)
            })
            .collect::<Vec<_>>();
        for channel in 0..nchannels {
            for frame in 0..nframes {
                output[nchannels * frame + channel] = buffers
                    .iter()
                    .map(|(buffer, constant_mask)| {
                        if (constant_mask & (1 << channel)) == 0 {
                            buffer[channel][frame]
                        } else {
                            buffer[channel][0]
                        }
                    })
                    .sum();
            }
        }

        self.steady_time += nframes as i64;

        let this_elapsed = this_start.elapsed();
        let elasped = this_elapsed.as_secs_f64();
        self.process_elaspeds.push(elasped);
        self.cpu_usages
            .push(elasped / (nframes as f64 / self.song.sample_rate));
        if self.process_elasped_last.elapsed() >= Duration::from_secs(1) {
            self.process_elasped_avg = self.process_elaspeds.iter().sum::<f64>()
                / self.process_elaspeds.len().max(1) as f64;
            self.cpu_usage =
                self.cpu_usages.iter().sum::<f64>() / self.cpu_usages.len().max(1) as f64;
            self.process_elaspeds.clear();
            self.cpu_usages.clear();
            self.process_elasped_last = Instant::now();
        }

        Ok(())
    }

    pub fn play(&mut self) {
        if self.play_p {
            return;
        }
        self.play_p = true;
    }

    pub fn plugin_delete(&mut self, track_index: usize, module_index: usize) -> anyhow::Result<()> {
        self.song.tracks[track_index].modules.remove(module_index);
        self.process_track_contexts[track_index]
            .plugins
            .remove(module_index);
        self.shmems[track_index].remove(module_index);
        self.sender_to_loop
            .send(MainToPlugin::Unload(track_index, module_index))?;
        Ok(())
    }

    pub fn song_close(&mut self) -> anyhow::Result<()> {
        for track_index in 0..self.song.tracks.len() {
            for module_index in 0..self.song.tracks[track_index].modules.len() {
                self.plugin_delete(track_index, module_index)?;
            }
        }
        self.song = Song::new();
        self.process_track_contexts.clear();
        self.shmems.clear();
        self.track_add();

        Ok(())
    }

    pub fn song_open(&mut self, song_file: String, hwnd: isize) -> anyhow::Result<()> {
        let file = File::open(&song_file)?;
        let reader = BufReader::new(file);
        let song = serde_json::from_reader(reader)?;

        self.song = song;

        let clap_manager = ClapManager::new();
        let mut xs = vec![];
        for track_index in 0..self.song.tracks.len() {
            self.process_track_contexts
                .push(ProcessTrackContext::default());
            self.shmems.push(vec![]);

            for module_index in 0..self.song.tracks[track_index].modules.len() {
                let module_id = self.song.tracks[track_index].modules[module_index]
                    .id
                    .clone();
                let description = clap_manager.description(&module_id);
                xs.push((track_index, description));
            }
        }

        for track_index in 0..self.song.tracks.len() {
            self.song.tracks[track_index].modules.clear();
        }

        for (track_index, description) in xs {
            self.plugin_load(track_index, description.unwrap(), false, hwnd)?;
        }

        self.song_file = Some(song_file);
        Ok(())
    }

    pub fn stop(&mut self) {
        if !self.play_p {
            return;
        }
        self.play_p = false;
        self.all_notef_off_p = true;
    }

    pub fn start_listener(singer: Arc<Mutex<Self>>, receiver: Receiver<SingerCommand>) {
        log::debug!("Song::start_listener");
        tokio::spawn(async move {
            singer_loop(singer, receiver).await.unwrap();
        });
    }

    fn send_song(&self) {
        self.song_sender
            .send(AppStateCommand::Song(self.song.clone()))
            .unwrap();
    }

    fn send_state(&self) {
        self.song_sender
            .send(AppStateCommand::State(SongState {
                song_file: self.song_file.clone(),
                play_p: self.play_p,
                line_play: self.line_play,
                loop_p: self.loop_p,
                loop_range: self.loop_range.clone(),
                process_elasped_avg: self.process_elasped_avg,
                cpu_usage: self.cpu_usage,
            }))
            .unwrap();
    }

    fn track_add(&mut self) {
        self.song.add_track();
        self.process_track_contexts
            .push(ProcessTrackContext::default());
        self.shmems.push(vec![]);
    }
}

async fn singer_loop(
    singer: Arc<Mutex<Singer>>,
    receiver: Receiver<SingerCommand>,
) -> anyhow::Result<()> {
    {
        let singer = singer.lock().unwrap();
        singer.send_song();
        singer.send_state();
    }

    while let Ok(msg) = receiver.recv() {
        log::debug!("Song 受信 {:?}", msg);
        match msg {
            SingerCommand::Play => {
                let mut singer = singer.lock().unwrap();
                singer.play();
                singer.send_state();
            }
            SingerCommand::Stop => {
                let mut singer = singer.lock().unwrap();
                singer.stop();
                singer.send_state();
            }
            SingerCommand::Loop => {
                let mut singer = singer.lock().unwrap();
                singer.loop_p = !singer.loop_p;
                singer.send_state();
            }
            SingerCommand::Song => singer.lock().unwrap().send_song(),
            SingerCommand::Note(cursor, note) => {
                let mut singer = singer.lock().unwrap();
                let song = &mut singer.song;
                if let Some(Some(lane)) = song
                    .tracks
                    .get_mut(cursor.track)
                    .map(|x| x.lanes.get_mut(cursor.lane))
                {
                    lane.notes.insert(note.line, note);
                    singer.send_song();
                }
            }
            SingerCommand::NoteDelete(cursor) => {
                let mut singer = singer.lock().unwrap();
                let song = &mut singer.song;
                if let Some(Some(lane)) = song
                    .tracks
                    .get_mut(cursor.track)
                    .map(|x| x.lanes.get_mut(cursor.lane))
                {
                    lane.notes.remove(&cursor.line);
                    singer.send_song();
                }
            }
            SingerCommand::PluginLoad(track_index, description, hwnd) => {
                log::debug!("will send MainToPlugin::Load {:?}", description);

                let mut singer = singer.lock().unwrap();
                singer.plugin_load(track_index, &description, true, hwnd)?;

                singer.send_song();
            }
            SingerCommand::PluginDelete(track_index, module_index) => {
                let mut singer = singer.lock().unwrap();
                singer.plugin_delete(track_index, module_index)?;

                singer.send_song();
            }
            SingerCommand::NoteOn(track_index, key, _channel, velocity, time) => {
                let mut singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .event_list_input
                    .push(Event::NoteOn(key, velocity, time));
            }
            SingerCommand::NoteOff(track_index, key, _channel, _velocity, time) => {
                let mut singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .event_list_input
                    .push(Event::NoteOff(key, time));
            }
            SingerCommand::TrackAdd => {
                let mut singer = singer.lock().unwrap();
                singer.track_add();
                singer.send_song();
            }
            SingerCommand::LaneAdd(track_index) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.lanes.push(Lane::new());
                    singer.send_song();
                }
            }
            SingerCommand::SongFile(song_file) => {
                let mut singer = singer.lock().unwrap();
                singer.song_file = Some(song_file);
                singer.send_state();
            }
            SingerCommand::SongOpen(song_file, hwnd) => {
                let mut singer = singer.lock().unwrap();
                singer.song_close()?;
                singer.song_open(song_file, hwnd)?;
                singer.send_song();
                singer.send_state();
            }
        }
    }
    Ok(())
}
