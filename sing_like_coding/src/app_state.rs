use std::{
    collections::VecDeque,
    env::current_exe,
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use arboard::Clipboard;
use clap_sys::id::clap_id;
use common::{
    dsp::{db_from_norm, db_to_norm},
    event::Event,
    module::{AudioInput, Module, ModuleIndex},
    plugin::{description::Description, param::Param},
    protocol::{MainToPlugin, PluginToMain},
    shmem::{open_shared_memory, SONG_STATE_NAME},
};
use eframe::egui::{ahash::HashMap, Color32};
use rfd::FileDialog;
use shared_memory::Shmem;

use crate::{
    command::{track_add::TrackAdd, Command},
    config::Config,
    midi_device::MidiDevice,
    model::{lane_item::LaneItem, note::Note, song::Song, track::Track},
    singer::{AudioToMain, MainToAudio},
    song_state::SongState,
    view::{
        root_view::Route,
        stereo_peak_meter::{DB_MAX, DB_MIN},
    },
};

pub enum UiCommand {
    Command,
    Digit(i32),
    FocusedPartNext,
    FocusedPartPrev,
    Follow,
    Lane(LaneCommand),
    LaneAdd,
    Loop,
    LoopRange,
    Mixer(MixerCommand),
    Module(ModuleCommand),
    PlayCursor,
    PlayToggle,
    RecToggle,
    Redo,
    SongSave,
    Track(TrackCommand),
    TrackAdd,
    TrackMute(Option<usize>, Option<bool>),
    TrackPan(usize, f32),
    TrackRecOn(usize),
    TrackRecOff(usize),
    TrackSolo(Option<usize>, Option<bool>),
    TrackVolume(usize, f32),
    Undo,
}

pub enum TrackCommand {
    Copy,
    CursorLeft,
    CursorRight,
    Cut,
    Delete,
    Dup,
    MoveLeft,
    MoveRight,
    Paste,
    Rename,
}

pub enum LaneCommand {
    AutomationParamSelect,
    Copy,
    Cut,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorUp,
    Go,
    Dup,
    LaneItemDelete,
    LaneItemMove(i64, i64),
    LaneItemUpdate(i16, i16, i16, bool, i16),
    Paste,
    SelectMode,
    SelectClear,
}

pub enum ModuleCommand {
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
    Delete,
    Open,
    Sidechain,
}

pub enum MixerCommand {
    CursorLeft,
    CursorRight,
    Pan(f32),
    Volume(f32),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CursorTrack {
    pub track: usize,
    pub lane: usize,
    pub line: usize,
}

impl CursorTrack {
    pub fn min_merge(&self, other: &Self) -> Self {
        let (track, lane) = if (self.track, self.lane) <= (other.track, other.lane) {
            (self.track, self.lane)
        } else {
            (other.track, other.lane)
        };
        Self {
            track,
            lane,
            line: self.line.min(other.line),
        }
    }

    pub fn max_merge(&self, other: &Self) -> Self {
        let (track, lane) = if (self.track, self.lane) >= (other.track, other.lane) {
            (self.track, self.lane)
        } else {
            (other.track, other.lane)
        };
        Self {
            track,
            lane,
            line: self.line.max(other.line),
        }
    }

    pub fn up(&self, _song: &Song) -> Self {
        let mut cursor = self.clone();
        if cursor.line != 0 {
            cursor.line -= 1;
        }
        cursor
    }

    pub fn down(&self, _song: &Song) -> Self {
        let mut cursor = self.clone();
        cursor.line += 1;
        cursor
    }

    pub fn left(&self, song: &Song) -> Self {
        let mut cursor = self.clone();
        if cursor.lane == 0 {
            if cursor.track == 0 {
                cursor.track = song.tracks.len() - 1;
            } else {
                cursor.track -= 1;
            }
            cursor.lane = song.tracks[cursor.track].lanes.len() - 1;
        } else {
            cursor.lane -= 1;
        }
        cursor
    }

    pub fn right(&self, song: &Song) -> Self {
        let mut cursor = self.clone();
        if cursor.lane == song.tracks[cursor.track].lanes.len() - 1 {
            cursor.lane = 0;
            if cursor.track + 1 == song.tracks.len() {
                cursor.track = 0;
            } else {
                cursor.track += 1;
            }
        } else {
            cursor.lane += 1;
        }
        cursor
    }

    pub fn move_by(&self, lane_delta: i64, line_delta: i64, song: &Song) -> Self {
        let mut cursor = self.clone();
        if lane_delta < 0 {
            for _ in 0..(lane_delta.abs()) {
                cursor = cursor.left(song);
            }
        } else {
            for _ in 0..lane_delta {
                cursor = cursor.right(song);
            }
        }
        if line_delta < 0 {
            for _ in 0..(line_delta.abs()) {
                cursor = cursor.down(song);
            }
        } else {
            for _ in 0..line_delta {
                cursor = cursor.up(song);
            }
        }
        cursor
    }
}

impl Ord for CursorTrack {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.track == other.track && self.lane == other.lane && self.line == other.line {
            std::cmp::Ordering::Equal
        } else if (self.track < other.track || self.track == other.track && self.lane <= other.lane)
            && self.line <= other.line
        {
            std::cmp::Ordering::Less
        } else {
            std::cmp::Ordering::Greater
        }
    }
}

impl PartialOrd for CursorTrack {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Default)]
pub struct CursorModule {
    pub index: usize,
}

#[derive(PartialEq)]
pub enum FocusedPart {
    Track,
    Lane,
    Module,
    Mixer,
}

pub struct AppState<'a> {
    pub config: Config,
    pub confirm_exit_popup_p: bool,
    pub now: Instant,
    pub elapsed: f32,
    digit: Option<i32>,
    pub focused_part: FocusedPart,
    pub follow_p: bool,
    pub cursor_track: CursorTrack,
    pub cursor_module: CursorModule,
    pub lane_item_last: LaneItem,
    midi_device_input: Option<MidiDevice>,
    pub rename_buffer: String,
    pub rename_request_focus_p: bool,
    pub rename_track_index: Option<usize>,
    pub route: Route,
    pub select_p: bool,
    pub selection_track_min: Option<CursorTrack>,
    pub selection_track_max: Option<CursorTrack>,
    pub song: Song,
    song_next: Option<Song>,
    song_apply_callbacks: VecDeque<Box<dyn Fn(&mut AppState) -> Result<()>>>,
    pub song_dirty_p: bool,
    sender_to_singer: Sender<MainToAudio>,
    receiver_from_audio: Receiver<AudioToMain>,
    sender_to_loop: Sender<MainToPlugin>,
    sender_midi: Sender<Event>,
    receiver_communicator_to_main_thread: Receiver<PluginToMain>,
    _song_state_shmem: Shmem,
    pub song_state: &'a SongState,
    callbacks_plugin_to_main: VecDeque<Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>>>,
    pub gui_context: Option<eframe::egui::Context>,

    pub param_select_view_params: Vec<Param>,

    // for MainView layout.
    pub offset_tracks: Vec<f32>,
    pub offset_flatten_lanes: Vec<f32>,
    pub width_lane: f32,
    pub flatten_lane_index_max: usize,
    pub flatten_lane_index_to_track_index_vec: Vec<usize>,
    pub track_lane_to_flatten_lane_index_map: HashMap<(usize, usize), usize>,
    pub flatten_lane_index_to_track_lane_vec: Vec<(usize, usize)>,
}

impl<'a> AppState<'a> {
    pub fn new(
        song: Song,
        sender_to_singer: Sender<MainToAudio>,
        receiver_from_audio: Receiver<AudioToMain>,
        sender_to_loop: Sender<MainToPlugin>,
        receiver_communicator_to_main_thread: Receiver<PluginToMain>,
        sender_midi: Sender<Event>,
    ) -> Self {
        let song_state_shmem = open_shared_memory::<SongState>(SONG_STATE_NAME).unwrap();
        let song_state = unsafe { &*(song_state_shmem.as_ptr() as *const SongState) };

        let mut this = Self {
            config: Config::load().unwrap_or_default(),
            confirm_exit_popup_p: false,
            now: Instant::now(),
            elapsed: 0.0,
            digit: None,
            focused_part: FocusedPart::Lane,
            follow_p: true,
            cursor_track: CursorTrack {
                track: 0,
                lane: 0,
                line: 0,
            },
            cursor_module: CursorModule { index: 0 },
            lane_item_last: LaneItem::default(),
            midi_device_input: None,
            rename_buffer: Default::default(),
            rename_track_index: None,
            rename_request_focus_p: false,
            route: Route::Track,
            select_p: false,
            selection_track_min: Default::default(),
            selection_track_max: Default::default(),
            song: song.clone(),
            song_next: Some(song),
            song_apply_callbacks: Default::default(),
            song_dirty_p: false,
            sender_to_singer,
            receiver_from_audio,
            sender_to_loop,
            sender_midi: sender_midi.clone(),
            receiver_communicator_to_main_thread,
            _song_state_shmem: song_state_shmem,
            song_state,
            callbacks_plugin_to_main: Default::default(),
            gui_context: None,

            param_select_view_params: vec![],

            offset_tracks: vec![],
            offset_flatten_lanes: vec![],
            width_lane: 1.0,
            flatten_lane_index_max: 0,
            flatten_lane_index_to_track_index_vec: Default::default(),
            track_lane_to_flatten_lane_index_map: Default::default(),
            flatten_lane_index_to_track_lane_vec: vec![],
        };

        if let Some(midi_device_input) = &this.config.midi_device_input {
            this.midi_device_input = Some(MidiDevice::new(midi_device_input, sender_midi).unwrap());
        }

        this
    }

    pub fn bpm_set(&mut self, bpm: f64) -> Result<()> {
        self.send_to_audio(MainToAudio::Bpm(bpm))?;
        Ok(())
    }

    pub fn color_cursor(&self) -> Color32 {
        if SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
            % 1000
            / 500
            == 0
        {
            Color32::from_rgb(0x00, 0x00, 0x88)
        } else {
            Color32::from_rgb(0x00, 0x00, 0x66)
        }
    }

    pub fn cursor_up(&mut self) {
        for _ in 0..self.digit.unwrap_or(1) {
            self.cursor_track = self.cursor_track.up(&self.song);
        }
    }

    pub fn cursor_down(&mut self) {
        for _ in 0..self.digit.unwrap_or(1) {
            self.cursor_track = self.cursor_track.down(&self.song);
        }
    }

    pub fn cursor_go(&mut self) {
        if let Some(bar) = self.digit {
            if bar > 0 {
                self.cursor_track.line = ((bar - 1) * 4 * 4) as usize;
            }
        }
    }

    pub fn cursor_left(&mut self) {
        for _ in 0..self.digit.unwrap_or(1) {
            self.cursor_track = self.cursor_track.left(&self.song);
        }
    }

    pub fn cursor_right(&mut self) {
        for _ in 0..self.digit.unwrap_or(1) {
            self.cursor_track = self.cursor_track.right(&self.song);
        }
    }

    fn loop_range(&mut self) -> Result<()> {
        if self.select_p {
            self.run_ui_command(&UiCommand::Lane(LaneCommand::SelectMode))?;
        }
        if let (Some(min), Some(max)) =
            (&mut self.selection_track_min, &mut self.selection_track_max)
        {
            let range = (min.line * 0x100)..((max.line + 1) * 0x100);
            self.send_to_audio(MainToAudio::LoopRange(range))?;
        }
        Ok(())
    }

    pub fn midi_device_input_open(&mut self, name: &str) -> Result<()> {
        self.midi_device_input = Some(MidiDevice::new(name, self.sender_midi.clone())?);
        self.config.midi_device_input = Some(name.to_string());
        self.config.save()?;
        Ok(())
    }

    fn module_at(&self, module_index: ModuleIndex) -> Option<&Module> {
        self.song.module_at(module_index)
    }

    fn module_at_mut(&mut self, module_index: ModuleIndex) -> Option<&mut Module> {
        self.song.module_at_mut(module_index)
    }

    fn module_load(&mut self, module_index: ModuleIndex, gui_open_p: bool) -> Result<()> {
        let module = self.module_at_mut(module_index);
        if module.is_none() {
            return Ok(());
        }
        let module = module.unwrap();
        let module_id = module.id;
        let plugin_id = module.plugin_id.clone();
        let state = module.state.take();
        self.send_to_plugin(
            MainToPlugin::Load(module_id, plugin_id, gui_open_p, state),
            // TODO singer にプラグインがアクティブになったことを通知？
            Box::new(|_, _| Ok(())),
        )?;
        Ok(())
    }

    pub fn now_update(&mut self) {
        self.elapsed = self.now.elapsed().as_secs_f32();
        self.now = Instant::now();
    }

    pub fn param_set(&mut self, module_index: usize, param_id: clap_id) -> Result<()> {
        self.send_to_audio(MainToAudio::PointNew(
            self.cursor_track,
            module_index,
            param_id,
        ))?;

        Ok(())
    }

    pub fn play(&mut self) -> Result<()> {
        self.send_to_audio(MainToAudio::Play)?;
        Ok(())
    }

    pub fn plugin_delete(&mut self, module_index: ModuleIndex) -> Result<()> {
        if let Some(module_id) = self.module_at(module_index).map(|x| x.id) {
            self.send_to_audio(MainToAudio::PluginDelete(module_index))?;
            self.send_to_plugin(MainToPlugin::Unload(module_id), Box::new(|_, _| Ok(())))?;
        }
        Ok(())
    }

    pub fn plugin_load(&mut self, description: &Description, gui_open_p: bool) -> Result<()> {
        match self.send_to_audio(MainToAudio::PluginLoad(
            self.cursor_track.track,
            description.id.clone(),
            description.name.clone(),
        ))? {
            AudioToMain::PluginLoad(_id, song) => {
                self.song = song;
                let module_index = self.song.tracks[self.cursor_track.track].modules.len() - 1;
                self.module_load((self.cursor_track.track, module_index), gui_open_p)?;
            }
            x => unreachable!("{:?}", x),
        }

        Ok(())
    }

    pub fn plugin_sidechain(
        &mut self,
        module_index: ModuleIndex,
        audio_input: AudioInput,
    ) -> Result<()> {
        self.send_to_audio(MainToAudio::PluginSidechain(module_index, audio_input))?;
        Ok(())
    }

    pub fn quit(&mut self) {
        let _ = self.send_to_audio(MainToAudio::Quit);
    }

    fn rec_set(&mut self, track_index: usize, value: bool) -> Result<()> {
        if value {
            self.send_to_audio(MainToAudio::TrackRecOn(track_index))?;
        } else {
            self.send_to_audio(MainToAudio::TrackRecOff(track_index))?;
        }
        Ok(())
    }

    pub fn receive_from_communicator(&mut self) -> Result<()> {
        while let Ok(mut message) = self.receiver_communicator_to_main_thread.try_recv() {
            match &mut message {
                PluginToMain::DidHwnd => {}
                PluginToMain::DidLoad(id, latency) => {
                    self.send_to_audio(MainToAudio::PluginLatency(*id, *latency))?;
                }
                PluginToMain::DidUnload(_) => {}
                PluginToMain::DidGuiOpen => {}
                PluginToMain::DidParams(_params) => {}
                PluginToMain::DidStateLoad => {}
                PluginToMain::DidStateSave(id, state) => {
                    if let Some(module) = self.song.module_by_id_mut(*id) {
                        module.state = Some(std::mem::take(state));
                    }
                }
                PluginToMain::DidScan => {}
                PluginToMain::Quit => {}
            }
            if let Some(callback) = self.callbacks_plugin_to_main.pop_front() {
                callback(self, message)?;
            }
        }
        Ok(())
    }

    fn redo(&mut self) -> Result<()> {
        self.send_to_audio(MainToAudio::Redo)?;
        Ok(())
    }

    pub fn run_ui_command(&mut self, command: &UiCommand) -> Result<()> {
        let digit = self.digit.clone();
        let track_index_last = self.cursor_track.track;
        match command {
            UiCommand::Command => {
                self.route = Route::Command;
            }
            UiCommand::Digit(digit) => {
                if let Some(x) = self.digit {
                    self.digit = Some(x * 10 + digit);
                } else {
                    self.digit = Some(*digit);
                }
            }
            UiCommand::Follow => {
                self.follow_p = !self.follow_p;
            }
            UiCommand::Loop => {
                self.send_to_audio(MainToAudio::Loop)?;
            }
            UiCommand::LoopRange => {
                self.loop_range()?;
            }
            UiCommand::FocusedPartNext => {
                self.focused_part = match self.focused_part {
                    FocusedPart::Track => FocusedPart::Lane,
                    FocusedPart::Lane => FocusedPart::Mixer,
                    FocusedPart::Mixer => FocusedPart::Module,
                    FocusedPart::Module => FocusedPart::Track,
                }
            }
            UiCommand::FocusedPartPrev => {
                self.focused_part = match self.focused_part {
                    FocusedPart::Track => FocusedPart::Module,
                    FocusedPart::Lane => FocusedPart::Track,
                    FocusedPart::Mixer => FocusedPart::Lane,
                    FocusedPart::Module => FocusedPart::Mixer,
                }
            }
            UiCommand::PlayCursor => {
                self.send_to_audio(MainToAudio::PlayLine(self.cursor_track.line))?;
            }
            UiCommand::PlayToggle => {
                if self.song_state.play_p {
                    self.send_to_audio(MainToAudio::Stop)?;
                } else {
                    self.send_to_audio(MainToAudio::Play)?;
                }
            }
            UiCommand::RecToggle => {
                self.send_to_audio(MainToAudio::RecToggle)?;
            }
            UiCommand::Redo => self.redo()?,
            UiCommand::SongSave => self.song_save()?,
            UiCommand::Track(command) => self.run_track_command(command)?,
            UiCommand::TrackAdd => {
                TrackAdd {}.call(self)?;
            }
            UiCommand::TrackMute(track_index, mute) => {
                let track_index = track_index.unwrap_or(self.cursor_track.track);
                let mute = mute.unwrap_or(!self.song.tracks[track_index].mute);
                self.send_to_audio(MainToAudio::TrackMute(track_index, mute))?;
            }
            UiCommand::TrackRecOn(track_index) => {
                self.rec_set(*track_index, true)?;
            }
            UiCommand::TrackRecOff(track_index) => {
                self.rec_set(*track_index, false)?;
            }
            UiCommand::TrackSolo(track_index, solo) => {
                let track_index = track_index.unwrap_or(self.cursor_track.track);
                let solo = solo.unwrap_or(!self.song.tracks[track_index].solo);
                self.send_to_audio(MainToAudio::TrackSolo(track_index, solo))?;
            }
            UiCommand::TrackPan(track_index, pan) => {
                self.send_to_audio(MainToAudio::TrackPan(*track_index, *pan))?;
            }
            UiCommand::TrackVolume(track_index, volume) => {
                self.send_to_audio(MainToAudio::TrackVolume(*track_index, *volume))?;
            }
            UiCommand::Undo => self.undo()?,
            UiCommand::LaneAdd => {
                self.send_to_audio(MainToAudio::LaneAdd(self.cursor_track.track))?;
            }
            UiCommand::Lane(LaneCommand::AutomationParamSelect) => {
                self.automation_param_select()?
            }
            UiCommand::Lane(LaneCommand::Copy) => self.lane_items_copy()?,
            UiCommand::Lane(LaneCommand::Cut) => self.late_items_cut()?,
            UiCommand::Lane(LaneCommand::Paste) => self.lane_items_paste()?,
            UiCommand::Lane(LaneCommand::CursorUp) => self.cursor_up(),
            UiCommand::Lane(LaneCommand::CursorDown) => self.cursor_down(),
            UiCommand::Lane(LaneCommand::CursorLeft) => self.cursor_left(),
            UiCommand::Lane(LaneCommand::CursorRight) => self.cursor_right(),
            UiCommand::Lane(LaneCommand::Go) => self.cursor_go(),
            UiCommand::Lane(LaneCommand::Dup) => self.lane_items_dup()?,
            UiCommand::Lane(LaneCommand::LaneItemDelete) => {
                self.send_to_audio(MainToAudio::LaneItem(vec![(
                    self.cursor_track.clone(),
                    None,
                )]))?;
            }
            UiCommand::Lane(LaneCommand::LaneItemMove(lane_delta, line_delta)) => {
                self.lane_items_move(*lane_delta, *line_delta)?;
            }
            UiCommand::Lane(LaneCommand::LaneItemUpdate(
                key_delta,
                velociy_delta,
                delay_delta,
                off,
                value_delta,
            )) => {
                self.lane_items_update(
                    *key_delta,
                    *velociy_delta,
                    *delay_delta,
                    *off,
                    *value_delta,
                )?;
            }
            UiCommand::Lane(LaneCommand::SelectMode) => {
                self.select_p = !self.select_p;
                if self.select_p {
                    self.selection_track_min = Some(self.cursor_track);
                    self.selection_track_max = None;
                } else if let Some(range) = self.lane_items_selection_range() {
                    // song の構造が変わっている場合もあるのでちゃんと if let Some する必要がある。
                    self.selection_track_min = Some(range.0);
                    self.selection_track_max = Some(range.1);
                }
            }
            UiCommand::Lane(LaneCommand::SelectClear) => {
                self.select_p = false;
                self.selection_track_min = None;
                self.selection_track_max = None;
            }
            UiCommand::Module(ModuleCommand::CursorUp) => {
                if self.cursor_module.index == 0 {
                    self.cursor_module.index =
                        self.song.tracks[self.cursor_track.track].modules.len();
                } else {
                    self.cursor_module.index -= 1;
                }
            }
            UiCommand::Module(ModuleCommand::CursorDown) => {
                if self.cursor_module.index
                    == self.song.tracks[self.cursor_track.track].modules.len()
                {
                    self.cursor_module.index = 0;
                } else {
                    self.cursor_module.index += 1;
                }
            }
            UiCommand::Module(ModuleCommand::CursorLeft) => {
                self.track_prev();
                if self.cursor_module.index
                    > self.song.tracks[self.cursor_track.track].modules.len()
                {
                    self.cursor_module.index =
                        self.song.tracks[self.cursor_track.track].modules.len();
                }
            }
            UiCommand::Module(ModuleCommand::CursorRight) => {
                self.track_next();
                if self.cursor_module.index
                    > self.song.tracks[self.cursor_track.track].modules.len()
                {
                    self.cursor_module.index =
                        self.song.tracks[self.cursor_track.track].modules.len();
                }
            }
            UiCommand::Module(ModuleCommand::Delete) => {
                if let Some(module_id) = self.module_at_cursort().map(|x| x.id) {
                    let track_index = self.cursor_track.track;
                    let module_index = self.cursor_module.index;
                    self.send_to_audio(MainToAudio::PluginDelete((track_index, module_index)))?;
                    self.send_to_plugin(MainToPlugin::Unload(module_id), Box::new(|_, _| Ok(())))?;
                }
            }
            UiCommand::Module(ModuleCommand::Open) => {
                if let Some(module) = self.module_at_cursort() {
                    self.send_to_plugin(MainToPlugin::GuiOpen(module.id), Box::new(|_, _| Ok(())))?;
                } else {
                    self.route = Route::PluginSelect;
                }
            }
            UiCommand::Module(ModuleCommand::Sidechain) => {
                if let Some(_module) = self.module_at_cursort() {
                    self.route = Route::SidechainSelect;
                }
            }
            UiCommand::Mixer(MixerCommand::CursorLeft) => self.track_prev(),
            UiCommand::Mixer(MixerCommand::CursorRight) => self.track_next(),
            UiCommand::Mixer(MixerCommand::Pan(delta)) => {
                if let Some(track) = self.track_current() {
                    self.send_to_audio(MainToAudio::TrackPan(
                        self.cursor_track.track,
                        (track.pan + (delta / 20.0)).clamp(0.0, 1.0),
                    ))?;
                }
            }
            UiCommand::Mixer(MixerCommand::Volume(delta)) => {
                if let Some(track) = self.track_current() {
                    let db = db_from_norm(track.volume, DB_MIN, DB_MAX) + delta;
                    self.send_to_audio(MainToAudio::TrackVolume(
                        self.cursor_track.track,
                        db_to_norm(db, DB_MIN, DB_MAX).clamp(0.0, 1.0),
                    ))?;
                }
            }
        }
        if self.digit == digit {
            self.digit = None;
        }

        let track_index = self.cursor_track.track;
        if track_index_last != track_index {
            if self.song_state.tracks[track_index_last].rec_p {
                self.send_to_audio(MainToAudio::TrackRecOff(track_index_last))?;
            }
            if !self.song_state.tracks[track_index].rec_p {
                self.send_to_audio(MainToAudio::TrackRecOn(track_index))?;
            }
        }

        Ok(())
    }

    fn send_to_audio(&mut self, command: MainToAudio) -> Result<AudioToMain> {
        self.sender_to_singer.send(command)?;
        let res = self.receiver_from_audio.recv()?;
        match res {
            AudioToMain::Song(song) => {
                self.song_dirty_p = true;
                self.song_next = Some(song);
                Ok(AudioToMain::Ok)
            }
            x => Ok(x),
        }
    }

    pub fn send_to_plugin(
        &mut self,
        command: MainToPlugin,
        callback: Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>>,
    ) -> Result<()> {
        self.callbacks_plugin_to_main.push_back(callback);
        self.sender_to_loop.send(command)?;
        Ok(())
    }

    pub fn song_next_apply(&mut self) -> Result<()> {
        if let Some(song) = self.song_next.take() {
            self.song = song;
            self.compute_track_offsets();
            self.cursor_track.track = self
                .cursor_track
                .track
                .min(self.song.tracks.len().saturating_sub(1));
            self.cursor_track.lane = self.cursor_track.lane.min(
                self.song.tracks[self.cursor_track.track]
                    .lanes
                    .len()
                    .saturating_sub(1),
            );
            self.cursor_module.index = self
                .cursor_module
                .index
                // + ボタンがあるので 1 引く必要はない
                .min(self.song.tracks[self.cursor_track.track].modules.len());
            if self.selection_track_min.is_some() {
                let track = self.selection_track_min.as_ref().unwrap().track;
                if track > self.song.tracks.len() - 1 {
                    self.selection_track_min = None;
                    self.selection_track_max = None;
                } else {
                    let track = track.min(self.song.tracks.len() - 1);
                    self.selection_track_min.as_mut().unwrap().track = track;
                    let lane = self.selection_track_min.as_ref().unwrap().lane;
                    self.selection_track_min.as_mut().unwrap().lane =
                        lane.min(self.song.tracks[track].lanes.len() - 1);
                }
            }
            if let Some(max) = self.selection_track_max.as_mut() {
                let track = max.track.min(self.song.tracks.len() - 1);
                max.track = track;
                max.lane = max.lane.min(self.song.tracks[track].lanes.len() - 1);
            }

            while let Some(callback) = self.song_apply_callbacks.pop_front() {
                callback(self)?;
            }
        }
        Ok(())
    }

    pub fn song_open(&mut self) -> Result<()> {
        if let Some(path) = FileDialog::new()
            .set_directory(song_directory())
            .pick_file()
        {
            self.send_to_audio(MainToAudio::SongOpen(
                path.to_str().map(|s| s.to_string()).unwrap(),
            ))?;
            self.song_apply_callbacks.push_back(Box::new(|state| {
                state.cursor_track = Default::default();
                state.cursor_module = Default::default();
                state.select_p = false;
                state.selection_track_min = Default::default();
                state.selection_track_max = Default::default();
                for track_index in 0..state.song.tracks.len() {
                    for module_index in 0..state.song.tracks[track_index].modules.len() {
                        state.module_load((track_index, module_index), false)?;
                    }
                }
                state.song_dirty_p = false;
                Ok(())
            }));
        }
        Ok(())
    }

    pub fn song_save(&mut self) -> Result<()> {
        let mut callback_p = false;
        let tracks_len = self.song.tracks.len();
        for track_index in 0..tracks_len {
            let modules_len = self.song.tracks[track_index].modules.len();
            for module_index in 0..modules_len {
                let callback: Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>> =
                    if track_index + 1 == tracks_len && module_index + 1 == modules_len {
                        callback_p = true;
                        Box::new(|state, _| state.song_save_file())
                    } else {
                        Box::new(|_, _| Ok(()))
                    };
                let module = &self.song.tracks[track_index].modules[module_index];
                self.send_to_plugin(MainToPlugin::StateSave(module.id), callback)?;
            }
        }
        if !callback_p {
            self.song_save_file()?;
        }

        Ok(())
    }

    pub fn compute_track_offsets(&mut self) {
        self.offset_tracks.clear();
        self.offset_flatten_lanes.clear();
        self.flatten_lane_index_to_track_index_vec.clear();
        self.track_lane_to_flatten_lane_index_map.clear();
        self.flatten_lane_index_to_track_lane_vec.clear();
        self.flatten_lane_index_max = 0;
        let mut acc = 0.0;
        for (track_index, track) in self.song.tracks.iter().enumerate() {
            self.offset_tracks.push(acc);
            for lane_index in 0..track.lanes.len() {
                self.offset_flatten_lanes.push(acc);
                acc += self.width_lane;
                self.flatten_lane_index_to_track_index_vec.push(track_index);
                self.track_lane_to_flatten_lane_index_map
                    .insert((track_index, lane_index), self.flatten_lane_index_max);
                self.flatten_lane_index_to_track_lane_vec
                    .push((track_index, lane_index));
                self.flatten_lane_index_max += 1;
            }
        }
        self.flatten_lane_index_max = self.flatten_lane_index_max.saturating_sub(1);
    }

    fn automation_param_select(&mut self) -> Result<()> {
        if self.song.tracks[self.cursor_track.track].modules.is_empty() {
            return Ok(());
        }
        self.route = Route::ParamSelect;
        Ok(())
    }

    fn lane_items_copy(&mut self) -> Result<()> {
        self.lane_items_copy_or_cut(true)
    }

    fn lane_items_copy_or_cut(&mut self, copy_p: bool) -> Result<()> {
        if self.select_p {
            self.run_ui_command(&UiCommand::Lane(LaneCommand::SelectMode))?;
        }
        let mut commands = vec![];
        if let (Some(min), Some(max)) = (&self.selection_track_min, &self.selection_track_max) {
            let mut itemss = vec![];
            for track_index in min.track..=max.track {
                if let Some(track) = self.song.tracks.get(track_index) {
                    let lane_start = if track_index == min.track {
                        min.lane
                    } else {
                        0
                    };
                    let lane_end = if track_index == max.track {
                        max.lane
                    } else {
                        track.lanes.len()
                    };
                    for lane_index in lane_start..=lane_end {
                        if let Some(lane) = track.lanes.get(lane_index) {
                            let mut items = vec![];
                            for line in min.line..=max.line {
                                items.push(lane.item(line).clone());
                                if !copy_p {
                                    commands.push((
                                        CursorTrack {
                                            track: track_index,
                                            lane: lane_index,
                                            line,
                                        },
                                        None,
                                    ));
                                }
                            }
                            itemss.push(items);
                        }
                    }
                }
            }
            let json = serde_json::to_string_pretty(&itemss)?;
            let mut clipboard = Clipboard::new().unwrap();
            clipboard.set_text(&json)?;
        } else if let Some(lane_item) = self.song.lane_item(&self.cursor_track) {
            let json = serde_json::to_string_pretty(&vec![vec![Some(lane_item)]])?;
            let mut clipboard = Clipboard::new().unwrap();
            clipboard.set_text(&json)?;
            if !copy_p {
                commands.push((self.cursor_track.clone(), None));
            }
        }

        self.send_to_audio(MainToAudio::LaneItem(commands))?;
        Ok(())
    }

    fn late_items_cut(&mut self) -> Result<()> {
        self.lane_items_copy_or_cut(false)
    }

    fn lane_items_dup(&mut self) -> Result<()> {
        if self.select_p {
            self.run_ui_command(&UiCommand::Lane(LaneCommand::SelectMode))?;
        }
        let mut commands = vec![];
        let itemss = self.lane_items_selected_cloned();
        if let (Some(min), Some(max)) =
            (&mut self.selection_track_min, &mut self.selection_track_max)
        {
            self.cursor_track.line = max.line + 1;
            let mut cursor = self.cursor_track.clone();
            for items in itemss.into_iter() {
                for item in items.into_iter() {
                    if let Some((_, item)) = item {
                        commands.push((cursor, Some(item)));
                    } else {
                        commands.push((cursor, None));
                    }
                    cursor = cursor.down(&self.song);
                }
                cursor = cursor.right(&self.song);
                cursor.line = self.cursor_track.line;
            }

            let line_delta = max.line - min.line + 1;
            min.line += line_delta;
            max.line += line_delta;
        } else if let Some(item) = self.song.lane_item(&self.cursor_track) {
            self.cursor_track.line += 1;
            commands.push((self.cursor_track.clone(), Some(item.clone())));
        }

        self.send_to_audio(MainToAudio::LaneItem(commands))?;
        Ok(())
    }

    fn lane_items_move(&mut self, lane_delta: i64, line_delta: i64) -> Result<()> {
        let mut commands = vec![];
        if self.selection_track_min.is_some() {
            let itemss = self.lane_items_selected_cloned();
            for (cursor, _) in itemss.clone().into_iter().flatten().filter_map(|x| x) {
                commands.push((cursor, None));
            }
            for (cursor, item) in itemss.into_iter().flatten().filter_map(|x| x) {
                let cursor = cursor.move_by(lane_delta, line_delta, &self.song);
                commands.push((cursor, Some(item)));
            }

            self.selection_track_min = self
                .selection_track_min
                .as_ref()
                .map(|x| x.move_by(lane_delta, line_delta, &self.song));
            self.selection_track_max = self
                .selection_track_max
                .as_ref()
                .map(|x| x.move_by(lane_delta, line_delta, &self.song));
        } else if let Some(item) = self.song.lane_item(&self.cursor_track) {
            commands.push((self.cursor_track.clone(), None));
            let cursor = self
                .cursor_track
                .move_by(lane_delta, line_delta, &self.song);
            commands.push((cursor, Some(item.clone())));
        }

        self.cursor_track = self
            .cursor_track
            .move_by(lane_delta, line_delta, &self.song);

        self.send_to_audio(MainToAudio::LaneItem(commands))?;
        Ok(())
    }

    fn lane_items_paste(&mut self) -> Result<()> {
        let mut commands = vec![];
        let mut clipboard = Clipboard::new().unwrap();
        if let Ok(text) = clipboard.get_text() {
            let mut cursor = self.cursor_track.clone();
            if let Ok(itemss) = serde_json::from_str::<Vec<Vec<Option<LaneItem>>>>(&text) {
                for items in itemss.into_iter() {
                    for item in items.into_iter() {
                        if let Some(item) = item {
                            commands.push((cursor, Some(item)));
                        } else {
                            commands.push((cursor, None));
                        }
                        cursor = cursor.down(&self.song);
                    }
                    cursor = cursor.right(&self.song);
                    cursor.line = self.cursor_track.line;
                }
            }
        }

        self.send_to_audio(MainToAudio::LaneItem(commands))?;
        Ok(())
    }

    fn lane_items_selected_cloned(&mut self) -> Vec<Vec<Option<(CursorTrack, LaneItem)>>> {
        let mut itemss = vec![];
        if let Some((min, max)) = self.lane_items_selection_range() {
            for track_index in min.track..=max.track {
                if let Some(track) = self.song.tracks.get(track_index) {
                    let lane_start = if track_index == min.track {
                        min.lane
                    } else {
                        0
                    };
                    let lane_end = if track_index == max.track {
                        max.lane
                    } else {
                        track.lanes.len()
                    };
                    for lane_index in lane_start..=lane_end {
                        if let Some(lane) = track.lanes.get(lane_index) {
                            let mut items = vec![];
                            for line in min.line..=max.line {
                                items.push(lane.item(line).cloned().map(|item| {
                                    (
                                        CursorTrack {
                                            track: track_index,
                                            lane: lane_index,
                                            line,
                                        },
                                        item,
                                    )
                                }));
                            }
                            itemss.push(items);
                        }
                    }
                }
            }
        }
        itemss
    }

    fn lane_items_selection_range(&self) -> Option<(CursorTrack, CursorTrack)> {
        if let Some(min) = &self.selection_track_min {
            let max = self
                .selection_track_max
                .as_ref()
                .unwrap_or(&self.cursor_track);
            Some((min.min_merge(max), min.max_merge(max)))
        } else {
            None
        }
    }

    fn lane_items_update(
        &mut self,
        key_delta: i16,
        velocity_delta: i16,
        delay_delta: i16,
        off: bool,
        value_delta: i16,
    ) -> Result<()> {
        let mut commands = vec![];
        if self.selection_track_min.is_some() {
            let itemss = self.lane_items_selected_cloned();
            for (cursor, mut lane_item) in itemss.into_iter().flatten().filter_map(|x| x) {
                match &mut lane_item {
                    LaneItem::Note(note) => {
                        if note.off {
                            continue;
                        }
                        note.key = (note.key + key_delta).clamp(0, 127);
                        note.velocity = (note.velocity + velocity_delta as f64).clamp(0.0, 127.0);
                        note.delay = (note.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
                    }
                    LaneItem::Point(point) => {
                        point.value = (point.value as i16 + value_delta).clamp(0, 0xff) as u8;
                    }
                }
                commands.push((cursor, Some(lane_item)));
            }
        } else {
            let lane_item = if let Some(mut lane_item) =
                self.song.lane_item(&self.cursor_track).cloned()
            {
                match &mut lane_item {
                    LaneItem::Note(note) => {
                        note.key = (note.key + key_delta).clamp(0, 127);
                        note.velocity = (note.velocity + velocity_delta as f64).clamp(0.0, 127.0);
                        note.delay = (note.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
                        note.off = off;
                    }
                    LaneItem::Point(point) => {
                        point.value = (point.value as i16 + value_delta).clamp(0, 0xff) as u8;
                    }
                }
                lane_item
            } else if off {
                LaneItem::Note(Note {
                    off: true,
                    ..Default::default()
                })
            } else {
                self.lane_item_last.clone()
            };

            commands.push((self.cursor_track.clone(), Some(lane_item.clone())));

            if let LaneItem::Note(Note { off: false, .. }) = lane_item {
                self.lane_item_last = lane_item;
            }
        }

        self.send_to_audio(MainToAudio::LaneItem(commands))?;
        Ok(())
    }

    pub fn loop_toggle(&mut self) -> Result<()> {
        self.send_to_audio(MainToAudio::Loop)?;
        Ok(())
    }

    fn module_at_cursort(&self) -> Option<&Module> {
        self.track_at_cursor()
            .and_then(|track| track.modules.get(self.cursor_module.index))
    }

    pub fn module_index_at_cursor(&self) -> ModuleIndex {
        (self.cursor_track.track, self.cursor_module.index)
    }

    pub fn run_track_command(&mut self, command: &TrackCommand) -> Result<()> {
        match command {
            TrackCommand::Copy => self.track_copy()?,
            TrackCommand::CursorLeft => self.track_prev(),
            TrackCommand::CursorRight => self.track_next(),
            TrackCommand::Cut => self.track_cut()?,
            TrackCommand::Delete => self.track_delete()?,
            TrackCommand::Dup => self.track_dup()?,
            TrackCommand::MoveLeft => self.track_move(-1)?,
            TrackCommand::MoveRight => self.track_move(1)?,
            TrackCommand::Paste => self.track_paste()?,
            TrackCommand::Rename => {
                self.rename_track_index = Some(self.cursor_track.track);
                self.rename_buffer = self.song.tracks[self.cursor_track.track].name.clone();
                self.rename_request_focus_p = true;
            }
        }
        Ok(())
    }

    fn song_save_file(&mut self) -> Result<()> {
        let song_file = if let Some(song_file) = &self.song_state.song_file_get() {
            song_file.into()
        } else {
            if let Some(path) = FileDialog::new()
                .set_directory(song_directory())
                .set_file_name(&self.song.name)
                .save_file()
            {
                self.send_to_audio(MainToAudio::SongFile(
                    path.to_str().map(|s| s.to_string()).unwrap(),
                ))?;
                path
            } else {
                return Ok(());
            }
        };
        let mut file = File::create(&song_file).unwrap();
        let json = serde_json::to_string_pretty(&self.song).unwrap();
        file.write_all(json.as_bytes()).unwrap();
        self.song_dirty_p = false;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.send_to_audio(MainToAudio::Stop)?;
        Ok(())
    }

    pub fn track_add(&mut self) -> Result<()> {
        self.send_to_audio(MainToAudio::TrackAdd)?;
        Ok(())
    }

    fn track_at_cursor(&self) -> Option<&Track> {
        self.song.tracks.get(self.cursor_track.track)
    }

    fn track_copy(&mut self) -> Result<()> {
        let track_index = self.cursor_track.track;
        let modules_len = self.song.tracks[track_index].modules.len();
        if modules_len == 0 {
            let json = serde_json::to_string_pretty(&self.song.tracks[track_index])?;
            let mut clipboard = Clipboard::new().unwrap();
            clipboard.set_text(&json)?;
        } else {
            for module_index in 0..modules_len {
                let callback: Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>> =
                    if module_index + 1 == modules_len {
                        Box::new(|state, _command| {
                            let json = serde_json::to_string_pretty(
                                &state.song.tracks[state.cursor_track.track],
                            )?;
                            let mut clipboard = Clipboard::new().unwrap();
                            clipboard.set_text(&json)?;
                            Ok(())
                        })
                    } else {
                        Box::new(|_state, _command| Ok(()))
                    };
                let module = &self.song.tracks[track_index].modules[module_index];
                self.send_to_plugin(MainToPlugin::StateSave(module.id), callback)?;
            }
        }

        Ok(())
    }

    fn track_cut(&mut self) -> Result<()> {
        self.track_copy()?;
        // main は消さない
        if self.cursor_track.track != 0 {
            self.track_delete()?;
        }
        Ok(())
    }

    fn track_delete(&mut self) -> Result<()> {
        // main は消さない
        if self.cursor_track.track != 0 {
            self.send_to_audio(MainToAudio::TrackDelete(self.cursor_track.track))?;
        }
        Ok(())
    }

    fn track_dup(&mut self) -> Result<()> {
        let track_index = self.cursor_track.track;
        let modules_len = self.track_at_cursor().unwrap().modules.len();
        if modules_len == 0 {
            self.send_to_audio(MainToAudio::TrackInsert(
                track_index + 1,
                self.track_at_cursor().unwrap().clone(),
            ))?;
            self.track_next();
        } else {
            for module_index in 0..modules_len {
                let callback: Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>> =
                    if module_index + 1 == modules_len {
                        Box::new(move |state, _command| {
                            state.send_to_audio(MainToAudio::TrackInsert(
                                track_index + 1,
                                state.song.tracks[track_index].clone(),
                            ))?;
                            state.song_apply_callbacks.push_back(Box::new(move |state| {
                                for module_index in
                                    0..state.song.tracks[track_index + 1].modules.len()
                                {
                                    state.module_load((track_index + 1, module_index), false)?;
                                }
                                state.track_next();
                                Ok(())
                            }));
                            Ok(())
                        })
                    } else {
                        Box::new(|_state, _command| Ok(()))
                    };
                self.send_to_plugin(
                    MainToPlugin::StateSave(
                        self.track_at_cursor().unwrap().modules[module_index].id,
                    ),
                    callback,
                )?;
            }
        }

        Ok(())
    }

    fn track_move(&mut self, delta: isize) -> Result<()> {
        if delta == 0 {
            return Ok(());
        }
        self.send_to_audio(MainToAudio::TrackMove(self.cursor_track.track, delta))?;
        self.song_apply_callbacks.push_back(Box::new(move |state| {
            state.cursor_track.track = state
                .cursor_track
                .track
                .saturating_add_signed(delta)
                .min(state.song.tracks.len() - 1);
            Ok(())
        }));
        Ok(())
    }

    fn track_paste(&mut self) -> Result<()> {
        if let Ok(text) = Clipboard::new()?.get_text() {
            if let Ok(track) = serde_json::from_str::<Track>(&text) {
                self.send_to_audio(MainToAudio::TrackInsert(self.cursor_track.track, track))?;
                self.song_apply_callbacks.push_back(Box::new(|state| {
                    let track =
                        &mut state.song_next.as_mut().unwrap().tracks[state.cursor_track.track];
                    let commands = track
                        .modules
                        .iter_mut()
                        .map(|module| {
                            MainToPlugin::Load(
                                module.id,
                                module.plugin_id.clone(),
                                false,
                                module.state.take(),
                            )
                        })
                        .collect::<Vec<_>>();

                    for command in commands {
                        state.send_to_plugin(
                            command,
                            // TODO singer にプラグインがアクティブになったことを通知？
                            Box::new(|_, _| Ok(())),
                        )?;
                    }
                    Ok(())
                }));
            }
        }
        Ok(())
    }

    fn track_current(&self) -> Option<&Track> {
        self.song.tracks.get(self.cursor_track.track)
    }

    fn track_next(&mut self) {
        if self.cursor_track.track == self.song.tracks.len() - 1 {
            self.cursor_track.track = 0;
        } else {
            self.cursor_track.track += 1;
        }
        self.cursor_track.lane = 0;
    }

    fn track_prev(&mut self) {
        if self.cursor_track.track == 0 {
            self.cursor_track.track = self.song.tracks.len() - 1;
        } else {
            self.cursor_track.track -= 1;
        }
        self.cursor_track.lane = 0;
    }

    pub fn track_rename(&mut self) -> Result<()> {
        if let Some(track_index) = self.rename_track_index.take() {
            self.send_to_audio(MainToAudio::TrackRename(
                track_index,
                self.rename_buffer.clone(),
            ))?;
        }
        Ok(())
    }

    fn undo(&mut self) -> Result<()> {
        self.send_to_audio(MainToAudio::Undo)?;
        Ok(())
    }
}

fn song_directory() -> PathBuf {
    let exe_path = current_exe().unwrap();
    let dir = exe_path.parent().unwrap();
    let dir = dir.join("user").join("song");
    create_dir_all(&dir).unwrap();
    dir
}
