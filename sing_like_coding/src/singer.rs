use std::{
    f32::consts::PI,
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
    app_state::{AppStateCommand, CursorTrack},
    model::{lane::Lane, lane_item::LaneItem, point::Point, song::Song, track::Track},
    song_state::SongState,
    util::next_id,
    view::stereo_peak_meter::DB_MIN,
};

use anyhow::Result;
use clap_sys::id::clap_id;
use common::{
    clap_manager::ClapManager,
    event::Event,
    module::Module,
    plugin_ref::PluginRef,
    process_data::{EventKind, ProcessData},
    process_track_context::ProcessTrackContext,
    protocol::MainToPlugin,
    shmem::{create_shared_memory, process_data_name, SONG_STATE_NAME},
};
use rayon::prelude::*;
use shared_memory::Shmem;

#[derive(Debug)]
pub enum SingerCommand {
    Play,
    Stop,
    Loop,
    Song,
    LaneItem(CursorTrack, LaneItem),
    LaneItemDelete(CursorTrack),
    #[allow(dead_code)]
    NoteOn(usize, i16, i16, f64, usize),
    #[allow(dead_code)]
    NoteOff(usize, i16, i16, f64, usize),
    PluginLatency(usize, u32),
    PluginLoad(usize, String, String),
    PluginDelete(usize, usize),
    PointNew(CursorTrack, usize, clap_id),
    TrackAdd,
    TrackDelete(usize),
    TrackInsert(usize, Track),
    TrackMute(usize, bool),
    TrackSolo(usize, bool),
    TrackPan(usize, f32),
    TrackVolume(usize, f32),
    LaneAdd(usize),
    SongFile(String),
    SongOpen(String),
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
    _song_state_shmem: Shmem,
    song_state_ptr: *mut SongState,
    sender_to_ui: Sender<AppStateCommand>,
    pub sender_to_plugin: Sender<MainToPlugin>,
    pub line_play: usize,
    process_track_contexts: Vec<Arc<Mutex<ProcessTrackContext>>>,
    shmems: Vec<Vec<Shmem>>,
    pub gui_context: Option<eframe::egui::Context>,

    cpu_usages: Vec<f64>,
    pub cpu_usage: f64,
    process_elaspeds: Vec<f64>,
    process_elasped_last: Instant,
    pub process_elasped_avg: f64,
}

unsafe impl Send for Singer {}
unsafe impl Sync for Singer {}

impl Singer {
    pub fn new(
        sender_to_ui: Sender<AppStateCommand>,
        sender_to_plugin: Sender<MainToPlugin>,
    ) -> Self {
        let song_state_shmem = create_shared_memory::<SongState>(SONG_STATE_NAME).unwrap();
        let song_state_ptr = song_state_shmem.as_ptr() as *mut SongState;
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
            _song_state_shmem: song_state_shmem,
            song_state_ptr,
            sender_to_ui,
            sender_to_plugin,
            line_play: 0,
            process_track_contexts: vec![],
            shmems: vec![],
            gui_context: None,

            cpu_usages: vec![],
            cpu_usage: 0.0,
            process_elaspeds: vec![],
            process_elasped_last: Instant::now(),
            process_elasped_avg: 0.0,
        };
        this.track_add();
        this.track_add();
        this.song_state_mut().init(&this);
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

    pub fn lane_item_set(&mut self, cursor: CursorTrack, lane_item: LaneItem) -> Result<()> {
        let song = &mut self.song;
        if let Some(Some(lane)) = song
            .tracks
            .get_mut(cursor.track)
            .map(|x| x.lanes.get_mut(cursor.lane))
        {
            lane.items.insert(cursor.line, lane_item);
        }
        Ok(())
    }

    pub fn plugin_latency_set(&mut self, id: usize, latency: u32) -> Result<()> {
        for context in self.process_track_contexts.iter_mut() {
            if let Some(plugin_ref) = context
                .lock()
                .unwrap()
                .plugins
                .iter_mut()
                .find(|plugin_ref| plugin_ref.id == id)
            {
                plugin_ref.latency = latency;
                break;
            }
        }

        Ok(())
    }

    pub fn plugin_load(
        &mut self,
        track_index: usize,
        clap_plugin_id: String,
        gui_open_p: bool,
    ) -> Result<usize> {
        let id = next_id();

        let shmem_name = process_data_name(id);
        let shmem = create_shared_memory::<ProcessData>(&shmem_name)?;

        self.process_track_contexts[track_index]
            .lock()
            .unwrap()
            .plugins
            .push(PluginRef::new(id, shmem.as_ptr() as *mut ProcessData)?);
        self.shmems[track_index].push(shmem);

        self.sender_to_plugin.send(MainToPlugin::Load(
            id,
            clap_plugin_id,
            track_index,
            gui_open_p,
        ))?;

        Ok(id)
    }

    pub fn process(&mut self, output: &mut [f32], nchannels: usize) -> Result<()> {
        let this_start = Instant::now();

        //log::debug!("AudioProcess process steady_time {}", self.steady_time);
        let nframes = output.len() / nchannels;

        self.compute_play_position(nframes);

        for track_index in 0..self.process_track_contexts.len() {
            let mut context = self.process_track_contexts[track_index].lock().unwrap();
            for module_index in 0..context.plugins.len() {
                let process_data = context.plugins[module_index].process_data_mut();
                process_data.nchannels = nchannels;
                process_data.nframes = nframes;
                process_data.play_p = if self.play_p { 1 } else { 0 };
                process_data.bpm = self.song.bpm;
                process_data.lpb = self.song.lpb;
                process_data.sample_rate = self.song.sample_rate;
                process_data.steady_time = self.steady_time;
                process_data.prepare();
            }

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

        // tracks process
        (1..self.song.tracks.len())
            .into_par_iter()
            .try_for_each(|track_index| {
                let track = &self.song.tracks[track_index];
                track.process(track_index, &self.process_track_contexts)
            })?;

        // prepare mixing paramss
        let mut data = self
            .process_track_contexts
            .iter_mut()
            .map(|x| {
                x.lock()
                    .unwrap()
                    .plugins
                    .last_mut()
                    .map(|plugin_ref: &mut PluginRef| {
                        let constant_mask = plugin_ref.process_data_mut().constant_mask_out;
                        (plugin_ref.ptr, constant_mask)
                    })
            })
            .zip(self.song.tracks.iter().map(|track| {
                if (track.pan - 0.5).abs() < 0.001 {
                    (
                        track.mute,
                        track.solo,
                        track.volume,
                        track.volume,
                        track.volume,
                    )
                } else {
                    let normalized_pan = (track.pan - 0.5) * 2.0;
                    let pan_angle = (normalized_pan + 1.0) * PI / 4.0;
                    (
                        track.mute,
                        track.solo,
                        track.volume * pan_angle.cos(),
                        track.volume * pan_angle.sin(),
                        track.volume,
                    )
                }
            }))
            .collect::<Vec<_>>();

        let mut buffers = data
            .iter()
            .enumerate()
            .map(|(track_index, x)| {
                x.0.map(|x| {
                    let process_data = unsafe { &mut *(x.0) };
                    if track_index == 0 {
                        &mut process_data.buffer_in
                    } else {
                        &mut process_data.buffer_out
                    }
                })
            })
            .collect::<Vec<_>>();

        let main_gains = [data[0].1 .2, data[0].1 .3, data[0].1 .4];

        // tracks pan pan volume
        for ((buffer_constant_mask, (_mute, _solo, gain_ch0, gain_ch1, gain_ch_restg)), buffer) in
            data[1..].iter_mut().zip(buffers.iter_mut())
        {
            if let (Some((_, constant_mask)), Some(buffer)) = (buffer_constant_mask, buffer) {
                for channel in 0..nchannels {
                    let constp = (*constant_mask & (1 << channel)) != 0;
                    let gain = if channel == 0 {
                        *gain_ch0
                    } else if channel == 1 {
                        *gain_ch1
                    } else {
                        *gain_ch_restg
                    };

                    if constp {
                        buffer[channel][0] *= gain;
                    } else {
                        for frame in 0..nframes {
                            buffer[channel][frame] *= gain;
                        }
                    }
                }
            }
        }

        // tracks mute solo -> main track
        let mut dummy = ProcessData::new();
        dummy.prepare();
        let dummy_p = self.song.tracks[0].modules.is_empty();
        let solo_any = self.song.tracks.iter().any(|t| t.solo);
        for frame in 0..nframes {
            for channel in 0..nchannels {
                let value = data[1..]
                    .iter()
                    .zip(buffers[1..].iter())
                    .map(|((buffer_constant_mask, (mute, solo, _, _, _)), buffer)| {
                        if let (Some((_, constant_mask)), Some(buffer)) =
                            (&buffer_constant_mask, buffer)
                        {
                            if *mute || (solo_any && !*solo) {
                                0.0
                            } else {
                                let constp = (*constant_mask & (1 << channel)) != 0;
                                if constp {
                                    buffer[channel][0]
                                } else {
                                    buffer[channel][frame]
                                }
                            }
                        } else {
                            0.0
                        }
                    })
                    .sum();
                if dummy_p {
                    dummy.buffer_out[channel][frame] = value;
                    dummy.constant_mask_out = 0;
                } else {
                    buffers[0].as_mut().unwrap()[channel][frame] = value;
                }
            }
        }

        // main track process
        if !dummy_p {
            self.song.tracks[0].process(0, &self.process_track_contexts)?;
        }

        let main_process_data = if dummy_p {
            &mut dummy
        } else {
            let ptr = self.process_track_contexts[0]
                .lock()
                .unwrap()
                .plugins
                .last()
                .unwrap()
                .ptr;
            let process_data = unsafe { &mut *(ptr) };
            process_data
        };

        // main track pan volume -> audio device
        let main_track = &self.song.tracks[0];
        for frame in 0..nframes {
            for channel in 0..nchannels {
                // いまは solo はいらない
                let value = if main_track.mute {
                    0.0
                } else {
                    let gain = if channel == 0 {
                        main_gains[0]
                    } else if channel == 1 {
                        main_gains[1]
                    } else {
                        main_gains[2]
                    };
                    let constp = (main_process_data.constant_mask_out & (1 << channel)) != 0;
                    if constp {
                        main_process_data.buffer_out[channel][0] *= gain;
                        main_process_data.buffer_out[channel][0]
                    } else {
                        main_process_data.buffer_out[channel][frame] *= gain;
                        main_process_data.buffer_out[channel][frame]
                    }
                };
                output[nchannels * frame + channel] = value;
            }
        }

        self.song_state_mut().param_track_index = usize::MAX;
        self.compute_song_state(main_process_data);

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

    pub fn plugin_delete(&mut self, track_index: usize, module_index: usize) -> Result<()> {
        self.song.tracks[track_index].modules.remove(module_index);
        self.process_track_contexts[track_index]
            .lock()
            .unwrap()
            .plugins
            .remove(module_index);
        self.shmems[track_index].remove(module_index);
        self.sender_to_plugin
            .send(MainToPlugin::Unload(track_index, module_index))?;
        Ok(())
    }

    pub fn point_new(
        &mut self,
        cursor: CursorTrack,
        module_index: usize,
        param_id: clap_id,
    ) -> Result<()> {
        let automation_params = &mut self.song.tracks[cursor.track].automation_params;
        let automation_params_index = if let Some(index) = automation_params
            .iter()
            .position(|x| *x == (module_index, param_id))
        {
            index
        } else {
            automation_params.push((module_index, param_id));
            automation_params.len() - 1
        };

        let point = Point {
            automation_params_index,
            value: 0,
            delay: 0,
        };
        self.lane_item_set(cursor, LaneItem::Point(point))?;

        Ok(())
    }

    pub fn song_close(&mut self) -> Result<()> {
        for track_index in (0..self.song.tracks.len()).rev() {
            for module_index in (0..self.song.tracks[track_index].modules.len()).rev() {
                self.plugin_delete(track_index, module_index)?;
            }
        }
        self.song = Song::new();
        self.process_track_contexts.clear();
        self.shmems.clear();
        self.track_add();

        Ok(())
    }

    pub fn song_open(&mut self, song_file: String) -> Result<()> {
        let file = File::open(&song_file)?;
        let reader = BufReader::new(file);
        let song = serde_json::from_reader(reader)?;

        self.song = song;

        let clap_manager = ClapManager::new();
        let mut xs = vec![];
        for track_index in 0..self.song.tracks.len() {
            self.process_track_contexts
                .push(Arc::new(Mutex::new(ProcessTrackContext::default())));
            self.shmems.push(vec![]);

            for module_index in 0..self.song.tracks[track_index].modules.len() {
                let module_id = self.song.tracks[track_index].modules[module_index]
                    .id
                    .clone();
                let description = clap_manager.description(&module_id);
                xs.push((track_index, description, module_index));
            }
        }

        for (track_index, description, module_index) in xs {
            self.plugin_load(track_index, description.unwrap().id.clone(), false)?;
            self.sender_to_plugin.send(MainToPlugin::StateLoad(
                track_index,
                module_index,
                self.song.tracks[track_index].modules[module_index]
                    .state
                    .take()
                    .unwrap(),
            ))?;
        }

        self.song_file = Some(song_file);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn song_state(&self) -> &SongState {
        unsafe { &*(self.song_state_ptr) }
    }

    pub fn song_state_mut(&self) -> &mut SongState {
        unsafe { &mut *(self.song_state_ptr) }
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

    fn compute_song_state(&mut self, main_process_data: &ProcessData) {
        let song_state = self.song_state_mut();

        for track_index in 0..self.process_track_contexts.len() {
            let context = self.process_track_contexts[track_index].lock().unwrap();
            if let Some(plugin_ref) = context.plugins.last() {
                let process_data = if track_index == 0 {
                    main_process_data
                } else {
                    plugin_ref.process_data()
                };
                for channel in 0..process_data.nchannels {
                    song_state.tracks[track_index].peaks[channel] = process_data.peak(channel);
                }
            } else {
                for channel in 0..2 {
                    song_state.tracks[track_index].peaks[channel] = DB_MIN;
                }
            }
        }

        'top: for (track_index, context) in self.process_track_contexts.iter().enumerate() {
            for (module_index, plugin) in context.lock().unwrap().plugins.iter().enumerate() {
                let process_data = plugin.process_data();
                for event_index in 0..process_data.nevents_output {
                    let event = &process_data.events_output[event_index];
                    if let common::process_data::Event {
                        kind: EventKind::ParamValue,
                        param_id,
                        ..
                    } = event
                    {
                        song_state.param_module_index = module_index;
                        song_state.param_id = *param_id;
                        song_state.param_track_index = track_index;
                        break 'top;
                    }
                }
            }
        }
    }

    fn send_song(&self) -> Result<()> {
        self.sender_to_ui
            .send(AppStateCommand::Song(self.song.clone()))?;
        self.gui_context.as_ref().map(|x| x.request_repaint());
        Ok(())
    }

    fn send_state(&self) {
        let state = self.song_state_mut();
        state.song_file_set(&self.song_file.clone().unwrap_or_default());
        state.play_p = self.play_p;
        state.line_play = self.line_play;
        state.loop_p = self.loop_p;
        state.loop_start = self.loop_range.start;
        state.loop_end = self.loop_range.end;
        state.process_elasped_avg = self.process_elasped_avg;
        state.cpu_usage = self.cpu_usage;
        self.gui_context.as_ref().map(|x| x.request_repaint());
    }

    fn track_add(&mut self) {
        self.song.track_add();
        self.process_track_contexts
            .push(Arc::new(Mutex::new(ProcessTrackContext::default())));
        self.shmems.push(vec![]);
    }

    fn track_delete(&mut self, track_index: usize) -> Result<()> {
        for module_index in (0..self.song.tracks[track_index].modules.len()).rev() {
            self.plugin_delete(track_index, module_index)?;
        }
        self.song.track_delete(track_index);
        self.process_track_contexts.remove(track_index);
        self.shmems.remove(track_index);
        Ok(())
    }

    fn track_insert(&mut self, track_index: usize, track: Track) -> Result<()> {
        self.song.track_insert(track_index, track);
        self.process_track_contexts.insert(
            track_index,
            Arc::new(Mutex::new(ProcessTrackContext::default())),
        );
        self.shmems.insert(track_index, vec![]);
        for module_index in 0..self.song.tracks[track_index].modules.len() {
            let clap_plugin_id = self.song.tracks[track_index].modules[module_index]
                .id
                .clone();
            self.plugin_load(track_index, clap_plugin_id, false)?;
            self.sender_to_plugin.send(MainToPlugin::StateLoad(
                track_index,
                module_index,
                self.song.tracks[track_index].modules[module_index]
                    .state
                    .take()
                    .unwrap(),
            ))?;
        }
        Ok(())
    }
}

async fn singer_loop(singer: Arc<Mutex<Singer>>, receiver: Receiver<SingerCommand>) -> Result<()> {
    {
        let singer = singer.lock().unwrap();
        singer.send_song()?;
        singer.send_state();
    }

    while let Ok(msg) = receiver.recv() {
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
            SingerCommand::Song => singer.lock().unwrap().send_song()?,
            SingerCommand::LaneItem(cursor, lane_item) => {
                let mut singer = singer.lock().unwrap();
                singer.lane_item_set(cursor, lane_item)?;
                singer.send_song()?;
            }
            SingerCommand::LaneItemDelete(cursor) => {
                let mut singer = singer.lock().unwrap();
                let song = &mut singer.song;
                if let Some(Some(lane)) = song
                    .tracks
                    .get_mut(cursor.track)
                    .map(|x| x.lanes.get_mut(cursor.lane))
                {
                    lane.items.remove(&cursor.line);
                    singer.send_song()?;
                }
            }
            SingerCommand::PluginLatency(id, latency) => {
                let mut singer = singer.lock().unwrap();
                singer.plugin_latency_set(id, latency)?;
                singer.send_song()?;
            }
            SingerCommand::PluginLoad(track_index, clap_plugin_id, name) => {
                let mut singer = singer.lock().unwrap();
                singer.plugin_load(track_index, clap_plugin_id.clone(), true)?;
                singer.song.tracks[track_index]
                    .modules
                    .push(Module::new(clap_plugin_id, name));

                singer.send_song()?;
            }
            SingerCommand::PluginDelete(track_index, module_index) => {
                let mut singer = singer.lock().unwrap();
                singer.plugin_delete(track_index, module_index)?;

                singer.send_song()?;
            }
            SingerCommand::PointNew(cursor, module_index, param_id) => {
                let mut singer = singer.lock().unwrap();
                singer.point_new(cursor, module_index, param_id)?;
                singer.send_song()?;
            }
            SingerCommand::NoteOn(track_index, key, _channel, velocity, delay) => {
                let singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .lock()
                    .unwrap()
                    .event_list_input
                    .push(Event::NoteOn(key, velocity, delay));
            }
            SingerCommand::NoteOff(track_index, key, _channel, _velocity, delay) => {
                let singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .lock()
                    .unwrap()
                    .event_list_input
                    .push(Event::NoteOff(key, delay));
            }
            SingerCommand::TrackAdd => {
                let mut singer = singer.lock().unwrap();
                singer.track_add();
                singer.send_song()?;
            }
            SingerCommand::TrackDelete(track_index) => {
                let mut singer = singer.lock().unwrap();
                singer.track_delete(track_index)?;
                singer.send_song()?;
            }
            SingerCommand::TrackInsert(track_index, track) => {
                let mut singer = singer.lock().unwrap();
                singer.track_insert(track_index, track)?;
                singer.send_song()?;
            }
            SingerCommand::TrackMute(track_index, mute) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.mute = mute;
                    singer.send_song()?;
                }
            }
            SingerCommand::TrackSolo(track_index, solo) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.solo = solo;
                    singer.send_song()?;
                }
            }
            SingerCommand::TrackPan(track_index, pan) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.pan = pan;
                    singer.send_song()?;
                }
            }
            SingerCommand::TrackVolume(track_index, volume) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.volume = volume;
                    singer.send_song()?;
                }
            }
            SingerCommand::LaneAdd(track_index) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.lanes.push(Lane::new());
                    singer.send_song()?;
                }
            }
            SingerCommand::SongFile(song_file) => {
                let mut singer = singer.lock().unwrap();
                singer.song_file = Some(song_file);
                singer.send_state();
            }
            SingerCommand::SongOpen(song_file) => {
                let mut singer = singer.lock().unwrap();
                singer.song_close()?;
                singer.song_open(song_file)?;
                singer.send_song()?;
                singer.send_state();
            }
        }
    }
    Ok(())
}
