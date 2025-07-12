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
        lane_item::LaneItem,
        point::Point,
        song::{topological_levels, Song},
        track::Track,
    },
    song_state::SongState,
    undo_history::UndoHistory,
    util::next_id,
    view::stereo_peak_meter::DB_MIN,
};

use anyhow::Result;
use clap_sys::{
    fixedpoint::{clap_beattime, clap_sectime},
    id::clap_id,
};
use common::{
    event::Event,
    module::{AudioInput, Module, ModuleIndex},
    plugin_ref::PluginRef,
    process_data::{EventKind, ProcessData},
    process_track_context::ProcessTrackContext,
    shmem::{create_shared_memory, process_data_name, SONG_STATE_NAME},
};
use rayon::prelude::*;
use shared_memory::Shmem;

#[derive(Clone, Debug)]
pub enum MainToAudio {
    Bpm(f64),
    Play,
    PlayLine(usize),
    Stop,
    Loop,
    LoopRange(Range<usize>),
    LaneAdd(usize),
    LaneItem(Vec<(CursorTrack, Option<LaneItem>)>),
    #[allow(dead_code)]
    NoteOn(usize, i16, i16, f64, usize),
    #[allow(dead_code)]
    NoteOff(usize, i16, i16, f64, usize),
    PluginLatency(usize, u32),
    PluginLoad(usize, String, String),
    PluginDelete(ModuleIndex),
    PluginSidechain(ModuleIndex, AudioInput),
    PointNew(CursorTrack, usize, clap_id),
    Quit,
    RecToggle,
    Redo,
    TrackAdd,
    TrackDelete(usize),
    TrackInsert(usize, Track),
    TrackMove(usize, isize),
    TrackMute(usize, bool),
    TrackSolo(usize, bool),
    TrackPan(usize, f32),
    TrackRecOn(usize),
    TrackRecOff(usize),
    TrackRename(usize, String),
    TrackVolume(usize, f32),
    Undo,
    #[allow(dead_code)]
    Song,
    SongFile(String),
    SongOpen(String),
}

#[derive(Debug)]
pub enum AudioToMain {
    Song(Song),
    Ok,
}

pub struct Singer {
    pub steady_time: i64,
    pub play_position: Range<usize>,
    play_position_start_last: usize,
    all_notef_off_p: bool,
    midi_buffer: Arc<Mutex<Vec<Event>>>,
    pub song: Song,
    _song_state_shmem: Shmem,
    song_state_ptr: *mut SongState,
    sender_to_main: Sender<AudioToMain>,
    process_track_contexts: Vec<Arc<Mutex<ProcessTrackContext>>>,
    shmems: Vec<Vec<Shmem>>,
    pub gui_context: Option<eframe::egui::Context>,

    cpu_usages: Vec<f64>,
    process_elaspeds: Vec<f64>,
    process_elasped_last: Instant,
}

unsafe impl Send for Singer {}
unsafe impl Sync for Singer {}

impl Singer {
    pub fn new(sender_to_main: Sender<AudioToMain>) -> Self {
        let song_state_shmem = create_shared_memory::<SongState>(SONG_STATE_NAME).unwrap();
        let song_state_ptr = song_state_shmem.as_ptr() as *mut SongState;
        let song = Song::new();
        let mut this = Self {
            steady_time: 0,
            play_position: 0..0,
            play_position_start_last: 0,
            all_notef_off_p: false,
            midi_buffer: Arc::new(Mutex::new(vec![])),
            song,
            _song_state_shmem: song_state_shmem,
            song_state_ptr,
            sender_to_main,
            process_track_contexts: vec![],
            shmems: vec![],
            gui_context: None,

            cpu_usages: vec![],
            process_elaspeds: vec![],
            process_elasped_last: Instant::now(),
        };
        this.track_add();
        this.track_add();
        this.song_state_mut().init();
        this
    }

    fn compute_play_position(&mut self, frames_count: usize) {
        let loop_p = self.song_state().loop_p;
        let loop_start = self.song_state().loop_start;
        if loop_p && self.play_position.end < loop_start {
            self.play_position.end = loop_start;
        }
        self.play_position.start = self.play_position.end;

        {
            let song_state = self.song_state_mut();
            let line = (self.play_position.start / 0x100) as usize;
            song_state.line_play = line;

            if !song_state.play_p {
                return;
            }
        }

        let sec_per_frame = frames_count as f64 / self.song.sample_rate;
        let sec_per_delay = 60.0 / (self.song.bpm * self.song.lpb as f64 * 256.0);
        let delta = (sec_per_frame / sec_per_delay).round() as usize;
        self.play_position.end = self.play_position.start + delta;

        let song_state = self.song_state_mut();
        if loop_p {
            if self.play_position.start < song_state.loop_start
                || song_state.loop_end <= self.play_position.start
            {
                self.play_position.start = song_state.loop_start;
                self.play_position.end = self.play_position.start + delta;
            } else if self.play_position.end > song_state.loop_end {
                let overflow = self.play_position.end - song_state.loop_end;
                self.play_position.end = song_state.loop_start + overflow;
            }
        }
    }

    fn lane_item_set(
        &mut self,
        cursor: CursorTrack,
        lane_item: Option<LaneItem>,
    ) -> Result<(CursorTrack, Option<LaneItem>)> {
        let song = &mut self.song;
        let mut result = (cursor.clone(), None);
        if let Some(track) = song.tracks.get_mut(cursor.track) {
            while track.lanes.len() - 1 < cursor.lane {
                track.lane_add();
            }
            let lane = &mut track.lanes[cursor.lane];
            result.1 = lane.items.remove(&cursor.line);
            if let Some(item) = lane_item {
                lane.items.insert(cursor.line, item);
            }
        }
        Ok(result)
    }

    fn lane_items_set(
        &mut self,
        items: Vec<(CursorTrack, Option<LaneItem>)>,
    ) -> Result<MainToAudio> {
        let mut undos = vec![];
        for (cursor, item) in items {
            undos.push(self.lane_item_set(cursor, item)?);
        }
        Ok(MainToAudio::LaneItem(undos))
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

        Ok(id)
    }

    pub fn process(&mut self, output: &mut [f32], nchannels: usize) -> Result<()> {
        let this_start = Instant::now();

        //log::debug!("AudioProcess process steady_time {}", self.steady_time);
        let nframes = output.len() / nchannels;

        self.compute_play_position(nframes);

        {
            let midi_buffer = {
                let mut x = self.midi_buffer.lock().unwrap();
                std::mem::take(&mut *x)
            };

            for track_index in 0..self.process_track_contexts.len() {
                let mut context = self.process_track_contexts[track_index].lock().unwrap();
                for module_index in 0..context.plugins.len() {
                    let process_data = context.plugins[module_index].process_data_mut();
                    let song_state = self.song_state();
                    process_data.nframes = nframes;
                    process_data.play_p = if song_state.play_p { 1 } else { 0 };
                    process_data.loop_p = if song_state.loop_p { 1 } else { 0 };
                    process_data.bpm = self.song.bpm;
                    process_data.lpb = self.song.lpb;
                    process_data.sample_rate = self.song.sample_rate;
                    process_data.steady_time = self.steady_time;
                    process_data.song_pos_beats = song_state.line_play as clap_beattime;
                    process_data.song_pos_seconds = (process_data.song_pos_beats as f64
                        * (60.0 / process_data.bpm))
                        as clap_sectime;
                    process_data.loop_start_beats = song_state.loop_start as i64 / 0x100;
                    process_data.loop_end_beats = song_state.loop_end as i64 / 0x100;
                    process_data.loop_start_seconds = (process_data.loop_start_beats as f64
                        * (60.0 / process_data.bpm))
                        as clap_sectime;
                    process_data.loop_end_seconds = (process_data.loop_end_beats as f64
                        * (60.0 / process_data.bpm))
                        as clap_sectime;
                    process_data.bar_number =
                        (process_data.song_pos_beats / self.song.lpb as i64) as i32;
                    process_data.bar_start = process_data.bar_number as i64 * self.song.lpb as i64;
                    process_data.prepare();
                }

                context.nchannels = nchannels;
                context.nframes = nframes;
                context.play_p = self.song_state().play_p;
                context.bpm = self.song.bpm;
                context.steady_time = self.steady_time;
                context.play_position = self.play_position.clone();
                let song_state = self.song_state();
                context.loop_range = song_state.loop_start..song_state.loop_end;
                context.prepare();

                if !midi_buffer.is_empty() {
                    if song_state.tracks[track_index].rec_p {
                        context.event_list_input.append(&mut midi_buffer.clone());
                        if song_state.rec_p {
                            self.song.tracks[track_index]
                                .events_append(&midi_buffer, &self.play_position)?;
                        }
                    }
                    self.song_state_mut().song_dirty_p = true;
                }

                if self.all_notef_off_p {
                    context.event_list_input.push(Event::NoteAllOff);
                }
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
            if let (Some((process_data, constant_mask, _nports_in, nports_out)), Some(buffer)) =
                (buffer_constant_mask, buffer)
            {
                let process_data = unsafe { &**process_data };
                for port in 0..*nports_out {
                    for channel in 0..process_data.nchannels_out[port] {
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
                        if let (Some((process_data, constant_mask, _, _)), Some(buffer)) =
                            (&buffer_constant_mask, buffer)
                        {
                            let process_data = unsafe { &**process_data };
                            let channel = channel % process_data.nchannels_out[0];
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
            let song_state = self.song_state_mut();
            song_state.process_elasped_avg = self.process_elaspeds.iter().sum::<f64>()
                / self.process_elaspeds.len().max(1) as f64;
            song_state.cpu_usage =
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
        self.song_state_mut().play_p = true;
        self.play_position.end = self.play_position_start_last;
    }

    pub fn play_line(&mut self, line: usize) {
        if self.song_state().play_p {
            return;
        }
        self.song_state_mut().play_p = true;
        let position = line * 0x100;
        self.play_position.end = position;
        self.play_position_start_last = position;
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
        self.lane_item_set(cursor, Some(LaneItem::Point(point)))?;

        Ok(())
    }

    fn rec_toggle(&mut self) {
        let song_state = self.song_state_mut();
        song_state.rec_p = !song_state.rec_p;
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

        self.song_state_mut().song_file_set(&song_file);
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
        tokio::spawn(async move {
            singer_loop(singer, receiver).await.unwrap();
        });
    }

    pub fn start_listener_midi(singer: Arc<Mutex<Self>>, receiver: Receiver<Event>) {
        let singer = singer.lock().unwrap();
        let midi_buffer = singer.midi_buffer.clone();
        tokio::spawn(async move {
            midi_loop(midi_buffer, receiver).await.unwrap();
        });
    }

    fn compute_song_state(&mut self, main_process_data: &ProcessData) {
        let song_state = self.song_state_mut();

        for track_index in 0..self.process_track_contexts.len() {
            let context = self.process_track_contexts[track_index].lock().unwrap();
            if let Some(plugin_ref) = context.plugins.last() {
                let process_data = plugin_ref.process_data();
                let nchannels = process_data.nchannels_out[0];
                for channel in 0..2 {
                    song_state.tracks[track_index].peaks[channel] =
                        process_data.peak(0, channel % nchannels);
                }
            } else if track_index == 0 {
                let process_data = main_process_data;
                let nchannels = process_data.nchannels_out[0];
                for channel in 0..process_data.nchannels_out[0] {
                    song_state.tracks[track_index].peaks[channel] =
                        process_data.peak(0, channel % nchannels);
                }
            } else {
                for channel in 0..2 {
                    song_state.tracks[track_index].peaks[channel] = DB_MIN;
                }
            }
        }

        // オートメンション対象のパラメータを特定するため
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
    let mut undo_history = UndoHistory::new();
    let mut break_p = false;
    while let Ok(msg) = receiver.recv() {
        let mut singer = singer.lock().unwrap();
        undo_history.traveling_p = false;
        if matches!(msg, MainToAudio::Quit) {
            break_p = true;
        }
        let response = run_main_to_audio(&mut singer, msg, &mut undo_history)?;
        if let AudioToMain::Song(_) = &response {
            singer.song_state_mut().song_dirty_p = false;
        }
        singer.sender_to_main.send(response)?;
        if break_p {
            break;
        }
    }
    log::debug!("singer loop quit.");
    Ok(())
}

fn run_main_to_audio(
    singer: &mut Singer,
    message: MainToAudio,
    undo_history: &mut UndoHistory,
) -> Result<AudioToMain> {
    let redo = message.clone();
    match message {
        MainToAudio::Bpm(bpm) => {
            singer.song.bpm = bpm;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::Play => {
            singer.play();
            Ok(AudioToMain::Ok)
        }
        MainToAudio::PlayLine(line) => {
            singer.play_line(line);
            Ok(AudioToMain::Ok)
        }
        MainToAudio::Stop => {
            singer.stop();
            Ok(AudioToMain::Ok)
        }
        MainToAudio::Loop => {
            singer.song_state_mut().loop_p = !singer.song_state().loop_p;
            Ok(AudioToMain::Ok)
        }
        MainToAudio::LoopRange(range) => {
            singer.song_state_mut().loop_start = range.start;
            singer.song_state_mut().loop_end = range.end;
            Ok(AudioToMain::Ok)
        }
        MainToAudio::Song => Ok(AudioToMain::Song(singer.song.clone())),
        MainToAudio::LaneItem(items) => {
            let undo = singer.lane_items_set(items)?;
            undo_history.add(undo, redo);
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::PluginLatency(id, latency) => {
            singer.plugin_latency_set(id, latency)?;
            Ok(AudioToMain::Ok)
        }
        MainToAudio::PluginLoad(track_index, clap_plugin_id, name) => {
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
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::PluginDelete(module_index) => {
            singer.plugin_delete(module_index)?;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::PluginSidechain(module_index, audio_input) => {
            singer.plugin_sidechain(module_index, audio_input)?;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::PointNew(cursor, module_index, param_id) => {
            singer.point_new(cursor, module_index, param_id)?;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::RecToggle => {
            singer.rec_toggle();
            Ok(AudioToMain::Ok)
        }
        MainToAudio::Redo => {
            if let Some(redo) = undo_history.redo() {
                run_main_to_audio(singer, redo, undo_history)?;
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::NoteOn(track_index, key, _channel, velocity, delay) => {
            singer.process_track_contexts[track_index]
                .lock()
                .unwrap()
                .event_list_input
                .push(Event::NoteOn(key, velocity, delay));
            Ok(AudioToMain::Ok)
        }
        MainToAudio::NoteOff(track_index, key, _channel, _velocity, delay) => {
            singer.process_track_contexts[track_index]
                .lock()
                .unwrap()
                .event_list_input
                .push(Event::NoteOff(key, delay));
            Ok(AudioToMain::Ok)
        }
        MainToAudio::TrackAdd => {
            singer.track_add();
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackDelete(track_index) => {
            singer.track_delete(track_index)?;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackInsert(track_index, track) => {
            singer.track_insert(track_index, track)?;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackMove(track_index, delta) => {
            singer.track_move(track_index, delta)?;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackMute(track_index, mute) => {
            if let Some(track) = singer.song.tracks.get_mut(track_index) {
                track.mute = mute;
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackSolo(track_index, solo) => {
            if let Some(track) = singer.song.tracks.get_mut(track_index) {
                track.solo = solo;
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackPan(track_index, pan) => {
            if let Some(track) = singer.song.tracks.get_mut(track_index) {
                track.pan = pan;
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackRecOn(track_index) => {
            singer.song_state_mut().tracks[track_index].rec_p = true;
            Ok(AudioToMain::Ok)
        }
        MainToAudio::TrackRecOff(track_index) => {
            singer.song_state_mut().tracks[track_index].rec_p = false;
            Ok(AudioToMain::Ok)
        }
        MainToAudio::TrackRename(track_index, name) => {
            if let Some(track) = singer.song.tracks.get_mut(track_index) {
                track.name = name;
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::TrackVolume(track_index, volume) => {
            if let Some(track) = singer.song.tracks.get_mut(track_index) {
                track.volume = volume;
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::Undo => {
            if let Some(undo) = undo_history.undo() {
                run_main_to_audio(singer, undo, undo_history)?;
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::LaneAdd(track_index) => {
            if let Some(track) = singer.song.tracks.get_mut(track_index) {
                track.lane_add();
            }
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::SongFile(song_file) => {
            singer.song_state_mut().song_file_set(&song_file);
            Ok(AudioToMain::Ok)
        }
        MainToAudio::SongOpen(song_file) => {
            singer.song_close()?;
            singer.song_open(song_file)?;
            Ok(AudioToMain::Song(singer.song.clone()))
        }
        MainToAudio::Quit => Ok(AudioToMain::Ok),
    }
}

async fn midi_loop(midi_buffer: Arc<Mutex<Vec<Event>>>, receiver: Receiver<Event>) -> Result<()> {
    while let Ok(event) = receiver.recv() {
        let mut midi_buffer = midi_buffer.lock().unwrap();
        midi_buffer.push(event);
    }
    Ok(())
}
