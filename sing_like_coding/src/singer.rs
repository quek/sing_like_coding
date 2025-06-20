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
    app_state::CursorTrack,
    model::{
        lane::Lane,
        lane_item::LaneItem,
        point::Point,
        song::{topological_levels, Song},
        track::Track,
    },
    song_state::SongState,
    util::next_id,
    view::stereo_peak_meter::DB_MIN,
};

use anyhow::Result;
use clap_sys::id::clap_id;
use common::{
    event::Event,
    module::{AudioInput, Module, ModuleId, ModuleIndex},
    plugin_ref::PluginRef,
    process_data::{EventKind, ProcessData},
    process_track_context::ProcessTrackContext,
    shmem::{create_shared_memory, process_data_name, SONG_STATE_NAME},
};
use rayon::prelude::*;
use shared_memory::Shmem;

#[derive(Debug)]
pub enum MainToAudio {
    Play,
    Stop,
    Loop,
    #[allow(dead_code)]
    Song,
    LaneItem(CursorTrack, LaneItem),
    LaneItemDelete(CursorTrack),
    #[allow(dead_code)]
    NoteOn(usize, i16, i16, f64, usize),
    #[allow(dead_code)]
    NoteOff(usize, i16, i16, f64, usize),
    PluginLatency(usize, u32),
    PluginLoad(usize, String, String),
    PluginDelete(ModuleIndex),
    PluginSidechain(ModuleIndex, AudioInput),
    PointNew(CursorTrack, usize, clap_id),
    TrackAdd,
    TrackDelete(usize),
    TrackInsert(usize, Track),
    TrackMove(usize, isize),
    TrackMute(usize, bool),
    TrackSolo(usize, bool),
    TrackPan(usize, f32),
    TrackVolume(usize, f32),
    LaneAdd(usize),
    SongFile(String),
    SongOpen(String),
}

#[derive(Debug)]
pub enum AudioToMain {
    PluginLoad(ModuleId, Song),
    Song(Song),
    Ok,
    Nop,
}

pub struct Singer {
    pub song_file: Option<String>,
    pub steady_time: i64,
    // pub play_p: bool,
    pub play_position: Range<usize>,
    // pub loop_p: bool,
    pub loop_range: Range<usize>,
    all_notef_off_p: bool,
    pub song: Song,
    _song_state_shmem: Shmem,
    song_state_ptr: *mut SongState,
    sender_to_main: Sender<AudioToMain>,
    // pub line_play: usize,
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
    pub fn new(sender_to_main: Sender<AudioToMain>) -> Self {
        let song_state_shmem = create_shared_memory::<SongState>(SONG_STATE_NAME).unwrap();
        let song_state_ptr = song_state_shmem.as_ptr() as *mut SongState;
        let song = Song::new();
        let mut this = Self {
            song_file: None,
            steady_time: 0,
            // play_p: false,
            play_position: 0..0,
            // loop_p: true,
            loop_range: 0..(0x100 * 0x20),
            all_notef_off_p: false,
            song,
            _song_state_shmem: song_state_shmem,
            song_state_ptr,
            sender_to_main,
            // line_play: 0,
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
        self.song_state_mut().line_play = line;

        if !self.song_state().play_p {
            return;
        }

        let sec_per_frame = frames_count as f64 / self.song.sample_rate;
        let sec_per_delay = 60.0 / (self.song.bpm * self.song.lpb as f64 * 256.0);
        self.play_position.end =
            self.play_position.start + (sec_per_frame / sec_per_delay).round() as usize;

        if self.song_state().loop_p {
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

    fn plugin_load(&mut self, track_index: usize) -> Result<usize> {
        let id = next_id();

        let shmem_name = process_data_name(id);
        let shmem = create_shared_memory::<ProcessData>(&shmem_name)?;

        self.process_track_contexts[track_index]
            .lock()
            .unwrap()
            .plugins
            .push(PluginRef::new(id, shmem.as_ptr() as *mut ProcessData)?);
        self.shmems[track_index].push(shmem);

        // self.sender_to_plugin
        //     .send(MainToPlugin::Load(id, clap_plugin_id, gui_open_p))?;

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
                process_data.play_p = if self.song_state().play_p { 1 } else { 0 };
                process_data.bpm = self.song.bpm;
                process_data.lpb = self.song.lpb;
                process_data.sample_rate = self.song.sample_rate;
                process_data.steady_time = self.steady_time;
                process_data.prepare();
            }

            context.nchannels = nchannels;
            context.nframes = nframes;
            context.play_p = self.song_state().play_p;
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
        for track_index in 0..self.song.tracks.len() {
            let mut context = self.process_track_contexts[track_index].lock().unwrap();
            self.song.tracks[track_index].compute_midi(&mut context);
        }
        // TODO topological_levels は必要な時だけ行う
        let levels = topological_levels(&self.song)?;
        for level in levels {
            level
                .into_par_iter()
                .try_for_each(|(track_index, module_index)| {
                    let track = &self.song.tracks[track_index];
                    let mut context = self.process_track_contexts[track_index].lock().unwrap();
                    track.process_module(
                        track_index,
                        &mut context,
                        module_index,
                        &self.process_track_contexts,
                    )
                })?;
        }

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
                        let pd = plugin_ref.process_data();
                        let constant_mask = pd.constant_mask_out;
                        (plugin_ref.ptr, constant_mask, pd.nports_in, pd.nports_out)
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
            data[1..].iter_mut().zip(buffers[1..].iter_mut())
        {
            if let (Some((_, constant_mask, _nports_in, nports_out)), Some(buffer)) =
                (buffer_constant_mask, buffer)
            {
                for port in 0..*nports_out {
                    for channel in 0..nchannels {
                        let constp = (constant_mask[port] & (1 << channel)) != 0;
                        let gain = if channel == 0 {
                            *gain_ch0
                        } else if channel == 1 {
                            *gain_ch1
                        } else {
                            *gain_ch_restg
                        };

                        if constp {
                            buffer[port][channel][0] *= gain;
                        } else {
                            for frame in 0..nframes {
                                buffer[port][channel][frame] *= gain;
                            }
                        }
                    }
                }
            }
        }

        // tracks mute solo -> main track
        let mut dummy = ProcessData::new();
        dummy.prepare();
        let dummy_p = self.song.tracks[0].modules.is_empty();

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

        let solo_any = self.song.tracks.iter().any(|t| t.solo);
        for frame in 0..nframes {
            for channel in 0..nchannels {
                let value = data[1..]
                    .iter()
                    .zip(buffers[1..].iter())
                    .map(|((buffer_constant_mask, (mute, solo, _, _, _)), buffer)| {
                        if let (Some((_, constant_mask, _, _)), Some(buffer)) =
                            (&buffer_constant_mask, buffer)
                        {
                            if *mute || (solo_any && !*solo) {
                                0.0
                            } else {
                                let constp = (constant_mask[0] & (1 << channel)) != 0;
                                if constp {
                                    buffer[0][channel][0]
                                } else {
                                    buffer[0][channel][frame]
                                }
                            }
                        } else {
                            0.0
                        }
                    })
                    .sum();

                if dummy_p {
                    main_process_data.buffer_out[0][channel][frame] = value;
                    main_process_data.constant_mask_out[0] = 0;
                } else {
                    main_process_data.buffer_in[0][channel][frame] = value;
                    main_process_data.constant_mask_in[0] = 0;
                }
            }
        }

        // main track process
        if !dummy_p {
            for module_index in 0..self.song.tracks[0].modules.len() {
                self.song.tracks[0].process_module(
                    0,
                    &mut self.process_track_contexts[0].lock().unwrap(),
                    module_index,
                    &self.process_track_contexts,
                )?;
            }
        }

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
                    let constp = (main_process_data.constant_mask_out[0] & (1 << channel)) != 0;
                    if constp {
                        main_process_data.buffer_out[0][channel][0] *= gain;
                        main_process_data.buffer_out[0][channel][0]
                    } else {
                        main_process_data.buffer_out[0][channel][frame] *= gain;
                        main_process_data.buffer_out[0][channel][frame]
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
        if self.song_state().play_p {
            return;
        }
        // self.play_p = true;
        self.song_state_mut().play_p = true;
    }

    pub fn plugin_delete(&mut self, module_index: ModuleIndex) -> Result<()> {
        let _module = self.song.tracks[module_index.0]
            .modules
            .remove(module_index.1);
        self.process_track_contexts[module_index.0]
            .lock()
            .unwrap()
            .plugins
            .remove(module_index.1);
        self.shmems[module_index.0].remove(module_index.1);
        Ok(())
    }

    pub fn plugin_sidechain(
        &mut self,
        module_index: ModuleIndex,
        audio_input: AudioInput,
    ) -> Result<()> {
        if let Some(module) = self.song.module_at_mut(module_index) {
            module.audio_inputs.push(audio_input);
        }
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
                self.plugin_delete((track_index, module_index))?;
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

        for track_index in 0..self.song.tracks.len() {
            self.process_track_contexts
                .push(Arc::new(Mutex::new(ProcessTrackContext::default())));
            self.shmems.push(vec![]);

            for module_index in 0..self.song.tracks[track_index].modules.len() {
                let id = self.plugin_load(track_index)?;
                let module = &mut self.song.tracks[track_index].modules[module_index];
                module.id = id;
            }
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
        if !self.song_state().play_p {
            return;
        }
        self.song_state_mut().play_p = false;
        self.all_notef_off_p = true;
    }

    pub fn start_listener(singer: Arc<Mutex<Self>>, receiver: Receiver<MainToAudio>) {
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
                    song_state.tracks[track_index].peaks[channel] = process_data.peak(0, channel);
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

    fn track_add(&mut self) {
        self.song.track_add();
        self.process_track_contexts
            .push(Arc::new(Mutex::new(ProcessTrackContext::default())));
        self.shmems.push(vec![]);
    }

    #[allow(dead_code)]
    fn track_at(&self, track_index: usize) -> Option<&Track> {
        self.song.track_at(track_index)
    }

    fn track_delete(&mut self, track_index: usize) -> Result<()> {
        for module_index in (0..self.song.tracks[track_index].modules.len()).rev() {
            self.plugin_delete((track_index, module_index))?;
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
            let id = self.plugin_load(track_index)?;
            let module = &mut self.song.tracks[track_index].modules[module_index];
            module.id = id;
        }
        Ok(())
    }

    fn track_move(&mut self, track_index: usize, delta: isize) -> Result<bool> {
        let track_index_new = track_index.saturating_add_signed(delta);
        if track_index_new == 0 || track_index_new >= self.song.tracks.len() {
            return Ok(false);
        }

        let context = self.process_track_contexts.remove(track_index);
        self.process_track_contexts.insert(track_index_new, context);
        let shmem = self.shmems.remove(track_index);
        self.shmems.insert(track_index_new, shmem);

        self.song.track_move(track_index, delta);

        Ok(true)
    }
}

async fn singer_loop(singer: Arc<Mutex<Singer>>, receiver: Receiver<MainToAudio>) -> Result<()> {
    while let Ok(msg) = receiver.recv() {
        match msg {
            MainToAudio::Play => {
                let mut singer = singer.lock().unwrap();
                singer.play();
                singer.sender_to_main.send(AudioToMain::Ok)?;
            }
            MainToAudio::Stop => {
                let mut singer = singer.lock().unwrap();
                singer.stop();
                singer.sender_to_main.send(AudioToMain::Ok)?;
            }
            MainToAudio::Loop => {
                let singer = singer.lock().unwrap();
                singer.song_state_mut().loop_p = !singer.song_state().loop_p;
                singer.sender_to_main.send(AudioToMain::Ok)?;
            }
            MainToAudio::Song => {
                let singer = singer.lock().unwrap();
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::LaneItem(cursor, lane_item) => {
                let mut singer = singer.lock().unwrap();
                singer.lane_item_set(cursor, lane_item)?;
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::LaneItemDelete(cursor) => {
                let mut singer = singer.lock().unwrap();
                let song = &mut singer.song;
                if let Some(Some(lane)) = song
                    .tracks
                    .get_mut(cursor.track)
                    .map(|x| x.lanes.get_mut(cursor.lane))
                {
                    lane.items.remove(&cursor.line);
                    singer
                        .sender_to_main
                        .send(AudioToMain::Song(singer.song.clone()))?;
                } else {
                    singer.sender_to_main.send(AudioToMain::Nop)?;
                }
            }
            MainToAudio::PluginLatency(id, latency) => {
                let mut singer = singer.lock().unwrap();
                singer.plugin_latency_set(id, latency)?;
                singer.sender_to_main.send(AudioToMain::Ok)?;
            }
            MainToAudio::PluginLoad(track_index, clap_plugin_id, name) => {
                let mut singer = singer.lock().unwrap();
                let id = singer.plugin_load(track_index)?;
                let track = &mut singer.song.tracks[track_index];
                let audio_inputs = if track.modules.is_empty() {
                    vec![]
                } else {
                    vec![AudioInput {
                        src_module_index: (track_index, track.modules.len() - 1),
                        src_port_index: 0,
                        dst_port_index: 0,
                    }]
                };
                singer.song.tracks[track_index].modules.push(Module::new(
                    id,
                    clap_plugin_id,
                    name,
                    audio_inputs,
                ));

                singer
                    .sender_to_main
                    .send(AudioToMain::PluginLoad(id, singer.song.clone()))?;
            }
            MainToAudio::PluginDelete(module_index) => {
                let mut singer = singer.lock().unwrap();
                singer.plugin_delete(module_index)?;
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::PluginSidechain(module_index, audio_input) => {
                let mut singer = singer.lock().unwrap();
                singer.plugin_sidechain(module_index, audio_input)?;
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::PointNew(cursor, module_index, param_id) => {
                let mut singer = singer.lock().unwrap();
                singer.point_new(cursor, module_index, param_id)?;
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::NoteOn(track_index, key, _channel, velocity, delay) => {
                let singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .lock()
                    .unwrap()
                    .event_list_input
                    .push(Event::NoteOn(key, velocity, delay));
                singer.sender_to_main.send(AudioToMain::Ok)?;
            }
            MainToAudio::NoteOff(track_index, key, _channel, _velocity, delay) => {
                let singer = singer.lock().unwrap();
                singer.process_track_contexts[track_index]
                    .lock()
                    .unwrap()
                    .event_list_input
                    .push(Event::NoteOff(key, delay));
                singer.sender_to_main.send(AudioToMain::Ok)?;
            }
            MainToAudio::TrackAdd => {
                let mut singer = singer.lock().unwrap();
                singer.track_add();
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::TrackDelete(track_index) => {
                let mut singer = singer.lock().unwrap();
                singer.track_delete(track_index)?;
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::TrackInsert(track_index, track) => {
                let mut singer = singer.lock().unwrap();
                singer.track_insert(track_index, track)?;
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::TrackMove(track_index, delta) => {
                let mut singer = singer.lock().unwrap();
                if singer.track_move(track_index, delta)? {}
            }
            MainToAudio::TrackMute(track_index, mute) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.mute = mute;
                }
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::TrackSolo(track_index, solo) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.solo = solo;
                }
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::TrackPan(track_index, pan) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.pan = pan;
                }
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::TrackVolume(track_index, volume) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.volume = volume;
                }
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::LaneAdd(track_index) => {
                let mut singer = singer.lock().unwrap();
                if let Some(track) = singer.song.tracks.get_mut(track_index) {
                    track.lanes.push(Lane::new());
                }
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
            MainToAudio::SongFile(song_file) => {
                let mut singer = singer.lock().unwrap();
                singer.song_file = Some(song_file);
                singer.sender_to_main.send(AudioToMain::Ok)?;
            }
            MainToAudio::SongOpen(song_file) => {
                let mut singer = singer.lock().unwrap();
                singer.song_close()?;
                singer.song_open(song_file)?;
                singer
                    .sender_to_main
                    .send(AudioToMain::Song(singer.song.clone()))?;
            }
        }
    }
    Ok(())
}
