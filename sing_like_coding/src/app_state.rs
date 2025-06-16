use std::{
    collections::VecDeque,
    env::current_exe,
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use arboard::Clipboard;
use common::{
    dsp::{db_from_norm, db_to_norm},
    module::Module,
    plugin::param::Param,
    protocol::{MainToPlugin, PluginToMain},
    shmem::{open_shared_memory, SONG_STATE_NAME},
};
use eframe::egui::{ahash::HashMap, Color32};
use rfd::FileDialog;
use shared_memory::Shmem;

use crate::{
    command::{track_add::TrackAdd, Command},
    model::{lane_item::LaneItem, note::Note, point::Point, song::Song, track::Track},
    singer::SingerCommand,
    song_state::SongState,
    view::{
        root_view::Route,
        stereo_peak_meter::{DB_MAX, DB_MIN},
    },
};

pub enum UiCommand {
    Command,
    FocusedPartNext,
    FocusedPartPrev,
    Follow,
    Lane(LaneCommand),
    LaneAdd,
    Loop,
    Mixer(MixerCommand),
    Module(ModuleCommand),
    PlayToggle,
    SongSave,
    Track(TrackCommand),
    TrackAdd,
    TrackMute(Option<usize>, Option<bool>),
    TrackPan(usize, f32),
    TrackSolo(Option<usize>, Option<bool>),
    TrackVolume(usize, f32),
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
}

pub enum LaneCommand {
    AutomationParamSelect,
    Copy,
    Cut,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorUp,
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
    pub focused_part: FocusedPart,
    pub follow_p: bool,
    pub cursor_track: CursorTrack,
    pub cursor_module: CursorModule,
    pub lane_item_last: LaneItem,
    pub route: Route,
    pub select_p: bool,
    pub selection_track_min: Option<CursorTrack>,
    pub selection_track_max: Option<CursorTrack>,
    pub song: Song,
    pub song_dirty_p: bool,
    pub sender_to_singer: Sender<SingerCommand>,
    sender_to_loop: Sender<MainToPlugin>,
    receiver_communicator_to_main_thread: Receiver<PluginToMain>,
    receiver_from_singer: Receiver<AppStateCommand>,
    song_open_p: bool,
    _song_state_shmem: Shmem,
    pub song_state: &'a SongState,
    callbacks_plugin_to_main: VecDeque<Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>>>,
    pub gui_context: Option<eframe::egui::Context>,

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
        sender_to_singer: Sender<SingerCommand>,
        sender_to_loop: Sender<MainToPlugin>,
        receiver_communicator_to_main_thread: Receiver<PluginToMain>,
        receiver_from_singer: Receiver<AppStateCommand>,
    ) -> Self {
        let song_state_shmem = open_shared_memory::<SongState>(SONG_STATE_NAME).unwrap();
        let song_state = unsafe { &*(song_state_shmem.as_ptr() as *const SongState) };

        Self {
            focused_part: FocusedPart::Lane,
            follow_p: true,
            cursor_track: CursorTrack {
                track: 0,
                lane: 0,
                line: 0,
            },
            cursor_module: CursorModule { index: 0 },
            lane_item_last: LaneItem::default(),
            route: Route::Track,
            select_p: false,
            selection_track_min: Default::default(),
            selection_track_max: Default::default(),
            song: Song::new(),
            song_dirty_p: false,
            sender_to_singer,
            sender_to_loop,
            receiver_communicator_to_main_thread,
            receiver_from_singer,
            song_open_p: false,
            _song_state_shmem: song_state_shmem,
            song_state,
            callbacks_plugin_to_main: Default::default(),
            gui_context: None,
            offset_tracks: vec![],
            offset_flatten_lanes: vec![],
            width_lane: 1.0,
            flatten_lane_index_max: 0,
            flatten_lane_index_to_track_index_vec: Default::default(),
            track_lane_to_flatten_lane_index_map: Default::default(),
            flatten_lane_index_to_track_lane_vec: vec![],
        }
    }

    pub fn color_cursor(&self, part: FocusedPart) -> Color32 {
        if self.focused_part != part
            || SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis()
                % 1000
                / 500
                == 0
        {
            // Color32::YELLOW
            Color32::from_rgba_premultiplied(0xff, 0xff, 0, 0xa0)
        } else {
            Color32::from_rgb(200, 200, 0)
        }
    }

    pub fn cursor_up(&mut self) {
        self.cursor_track = self.cursor_track.up(&self.song);
    }

    pub fn cursor_down(&mut self) {
        self.cursor_track = self.cursor_track.down(&self.song);
    }

    pub fn cursor_left(&mut self) {
        self.cursor_track = self.cursor_track.left(&self.song);
    }

    pub fn cursor_right(&mut self) {
        self.cursor_track = self.cursor_track.right(&self.song);
    }

    pub fn module_mut(&mut self, track_index: usize, module_index: usize) -> Option<&mut Module> {
        self.song
            .tracks
            .get_mut(track_index)
            .and_then(|x| x.modules.get_mut(module_index))
    }

    pub fn param_set(&mut self, param: Param) -> Result<()> {
        if let Some(LaneItem::Point(point)) = self.song.lane_item(&self.cursor_track) {
            let mut point = point.clone();
            point.param_id = param.id;
            self.sender_to_singer.send(SingerCommand::LaneItem(
                self.cursor_track,
                LaneItem::Point(point),
            ))?;
        }
        Ok(())
    }

    pub fn receive_from_singer(&mut self) -> Result<()> {
        while let Ok(command) = self.receiver_from_singer.try_recv() {
            match command {
                AppStateCommand::Song(song) => {
                    if self.song_open_p {
                        self.song_open_did(song).unwrap();
                        self.song_open_p = false;
                        self.song_dirty_p = false;
                    } else {
                        self.song = song;
                        self.song_dirty_p = true;
                    }
                    self.compute_track_offsets();
                }
                AppStateCommand::Quit => (),
            }
        }

        Ok(())
    }

    pub fn receive_from_communicator(&mut self) -> Result<()> {
        while let Ok(mut message) = self.receiver_communicator_to_main_thread.try_recv() {
            match &mut message {
                PluginToMain::DidStateSave(track_index, module_index, state) => {
                    if let Some(module) = self.module_mut(*track_index, *module_index) {
                        module.state = Some(std::mem::take(state));
                    }
                }
                _ => {}
            }
            if let Some(callback) = self.callbacks_plugin_to_main.pop_front() {
                callback(self, message)?;
            }
        }
        Ok(())
    }

    pub fn run_ui_command(&mut self, command: &UiCommand) -> Result<()> {
        match command {
            UiCommand::Command => {
                self.route = Route::Command;
            }
            UiCommand::Follow => {
                self.follow_p = !self.follow_p;
            }
            UiCommand::Loop => {
                self.sender_to_singer.send(SingerCommand::Loop)?;
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
            UiCommand::PlayToggle => {
                if self.song_state.play_p {
                    self.sender_to_singer.send(SingerCommand::Stop)?;
                } else {
                    self.sender_to_singer.send(SingerCommand::Play)?;
                }
            }
            UiCommand::SongSave => {
                self.song_save()?;
            }

            UiCommand::Track(command) => self.run_track_command(command)?,

            UiCommand::TrackAdd => {
                TrackAdd {}.call(self)?;
            }
            UiCommand::TrackMute(track_index, mute) => {
                let track_index = track_index.unwrap_or(self.cursor_track.track);
                let mute = mute.unwrap_or(!self.song.tracks[track_index].mute);
                self.sender_to_singer
                    .send(SingerCommand::TrackMute(track_index, mute))?;
            }
            UiCommand::TrackSolo(track_index, solo) => {
                let track_index = track_index.unwrap_or(self.cursor_track.track);
                let solo = solo.unwrap_or(!self.song.tracks[track_index].solo);
                self.sender_to_singer
                    .send(SingerCommand::TrackSolo(track_index, solo))?;
            }
            UiCommand::TrackPan(track_index, pan) => self
                .sender_to_singer
                .send(SingerCommand::TrackPan(*track_index, *pan))?,
            UiCommand::TrackVolume(track_index, volume) => self
                .sender_to_singer
                .send(SingerCommand::TrackVolume(*track_index, *volume))?,
            UiCommand::LaneAdd => self
                .sender_to_singer
                .send(SingerCommand::LaneAdd(self.cursor_track.track))?,
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
            UiCommand::Lane(LaneCommand::Dup) => self.lane_items_dup()?,
            UiCommand::Lane(LaneCommand::LaneItemDelete) => self
                .sender_to_singer
                .send(SingerCommand::LaneItemDelete(self.cursor_track.clone()))?,
            UiCommand::Lane(LaneCommand::LaneItemMove(lane_delta, line_delta)) => {
                self.lane_items_move(*lane_delta, *line_delta)?;
            }
            UiCommand::Lane(LaneCommand::LaneItemUpdate(
                key_delta,
                velociy_delta,
                delay_delta,
                off,
                module_delta,
            )) => {
                self.lane_items_update(
                    *key_delta,
                    *velociy_delta,
                    *delay_delta,
                    *off,
                    *module_delta,
                )?;
            }
            UiCommand::Lane(LaneCommand::SelectMode) => {
                self.select_p = !self.select_p;
                if self.select_p {
                    self.selection_track_min = Some(self.cursor_track);
                    self.selection_track_max = None;
                } else {
                    let range = self.lane_items_selection_range().unwrap();
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
            UiCommand::Mixer(MixerCommand::CursorLeft) => self.track_prev(),
            UiCommand::Mixer(MixerCommand::CursorRight) => self.track_next(),
            UiCommand::Mixer(MixerCommand::Pan(delta)) => {
                if let Some(track) = self.track_current() {
                    self.sender_to_singer.send(SingerCommand::TrackPan(
                        self.cursor_track.track,
                        (track.pan + (delta / 20.0)).clamp(0.0, 1.0),
                    ))?;
                }
            }
            UiCommand::Mixer(MixerCommand::Volume(delta)) => {
                if let Some(track) = self.track_current() {
                    let db = db_from_norm(track.volume, DB_MIN, DB_MAX) + delta;
                    self.sender_to_singer.send(SingerCommand::TrackVolume(
                        self.cursor_track.track,
                        db_to_norm(db, DB_MIN, DB_MAX).clamp(0.0, 1.0),
                    ))?;
                }
            }
        }
        Ok(())
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

    pub fn song_open(&mut self) -> Result<()> {
        if let Some(path) = FileDialog::new()
            .set_directory(song_directory())
            .pick_file()
        {
            self.song_open_p = true;
            self.sender_to_singer.send(SingerCommand::SongOpen(
                path.to_str().map(|s| s.to_string()).unwrap(),
            ))?;
        }
        Ok(())
    }

    pub fn song_open_did(&mut self, song: Song) -> Result<()> {
        self.song = song;
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
                self.send_to_plugin(MainToPlugin::StateSave(track_index, module_index), callback)?;
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
        if let Some(LaneItem::Point(point)) = self.song.lane_item(&self.cursor_track) {
            let callback: Box<dyn Fn(&mut AppState, PluginToMain) -> Result<()>> =
                Box::new(|state, command| {
                    match command {
                        PluginToMain::DidParams(params) => {
                            state.route = Route::ParamSelect(params);
                        }
                        _ => {}
                    }
                    Ok(())
                });
            self.send_to_plugin(
                MainToPlugin::Params(self.cursor_track.track, point.module_index),
                callback,
            )?;
        }
        Ok(())
    }

    fn lane_items_copy(&mut self) -> Result<()> {
        self.lane_items_copy_or_cut(true)
    }

    fn lane_items_copy_or_cut(&mut self, copy_p: bool) -> Result<()> {
        if self.select_p {
            self.run_ui_command(&UiCommand::Lane(LaneCommand::SelectMode))?;
        }
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
                                    self.sender_to_singer.send(SingerCommand::LaneItemDelete(
                                        CursorTrack {
                                            track: track_index,
                                            lane: lane_index,
                                            line,
                                        },
                                    ))?;
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
        }

        Ok(())
    }

    fn late_items_cut(&mut self) -> Result<()> {
        self.lane_items_copy_or_cut(false)
    }

    fn lane_items_dup(&mut self) -> Result<()> {
        if self.select_p {
            self.run_ui_command(&UiCommand::Lane(LaneCommand::SelectMode))?;
        }
        let itemss = self.lane_items_selected_cloned();
        if let (Some(min), Some(max)) =
            (&mut self.selection_track_min, &mut self.selection_track_max)
        {
            self.cursor_track.line = max.line + 1;
            let mut cursor = self.cursor_track.clone();
            for items in itemss.into_iter() {
                for item in items.into_iter() {
                    if let Some((_, item)) = item {
                        self.sender_to_singer
                            .send(SingerCommand::LaneItem(cursor, item))?;
                    } else {
                        self.sender_to_singer
                            .send(SingerCommand::LaneItemDelete(cursor))?;
                    }
                    cursor = cursor.down(&self.song);
                }
                cursor = cursor.right(&self.song);
                cursor.line = self.cursor_track.line;
            }

            let line_delta = max.line - min.line + 1;
            min.line += line_delta;
            max.line += line_delta;
        }

        Ok(())
    }

    fn lane_items_move(&mut self, lane_delta: i64, line_delta: i64) -> Result<()> {
        if self.selection_track_min.is_some() {
            let itemss = self.lane_items_selected_cloned();
            for (cursor, _) in itemss.clone().into_iter().flatten().filter_map(|x| x) {
                self.sender_to_singer
                    .send(SingerCommand::LaneItemDelete(cursor))?;
            }
            for (cursor, item) in itemss.into_iter().flatten().filter_map(|x| x) {
                let cursor = cursor.move_by(lane_delta, line_delta, &self.song);
                self.sender_to_singer
                    .send(SingerCommand::LaneItem(cursor, item))?;
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
            self.sender_to_singer
                .send(SingerCommand::LaneItemDelete(self.cursor_track.clone()))?;
            let cursor = self
                .cursor_track
                .move_by(lane_delta, line_delta, &self.song);
            self.sender_to_singer
                .send(SingerCommand::LaneItem(cursor, item.clone()))?;
        }

        self.cursor_track = self
            .cursor_track
            .move_by(lane_delta, line_delta, &self.song);

        Ok(())
    }

    fn lane_items_paste(&mut self) -> Result<()> {
        let mut clipboard = Clipboard::new().unwrap();
        if let Ok(text) = clipboard.get_text() {
            let mut cursor = self.cursor_track.clone();
            if let Ok(itemss) = serde_json::from_str::<Vec<Vec<Option<LaneItem>>>>(&text) {
                for items in itemss.into_iter() {
                    for item in items.into_iter() {
                        if let Some(item) = item {
                            self.sender_to_singer
                                .send(SingerCommand::LaneItem(cursor, item))?;
                        } else {
                            self.sender_to_singer
                                .send(SingerCommand::LaneItemDelete(cursor))?;
                        }
                        cursor = cursor.down(&self.song);
                    }
                    cursor = cursor.right(&self.song);
                    cursor.line = self.cursor_track.line;
                }
            }
        }

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
        key_or_param_id_delta: i16,
        velocity_or_value_delta: i16,
        delay_delta: i16,
        off: bool,
        module_delta: i16,
    ) -> Result<()> {
        if self.selection_track_min.is_some() {
            let itemss = self.lane_items_selected_cloned();
            for (cursor, mut lane_item) in itemss.into_iter().flatten().filter_map(|x| x) {
                match &mut lane_item {
                    LaneItem::Note(note) => {
                        if note.off {
                            continue;
                        }
                        note.key = (note.key + key_or_param_id_delta).clamp(0, 127);
                        note.velocity =
                            (note.velocity + velocity_or_value_delta as f64).clamp(0.0, 127.0);
                        note.delay = (note.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
                    }
                    LaneItem::Point(point) => {
                        // a01 00 00
                        // module_index+param_id value delay
                        point.module_index = point
                            .module_index
                            .saturating_add_signed(module_delta as isize);
                        point.param_id = point
                            .param_id
                            .saturating_add_signed(key_or_param_id_delta as i32);
                        point.value =
                            (point.value as i16 + velocity_or_value_delta).clamp(0, 0xff) as u8;
                        point.delay = (point.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
                    }
                }
                self.sender_to_singer
                    .send(SingerCommand::LaneItem(cursor, lane_item))
                    .unwrap();
            }
        } else {
            let lane_item =
                if let Some(mut lane_item) = self.song.lane_item(&self.cursor_track).cloned() {
                    match &mut lane_item {
                        LaneItem::Note(note) => {
                            note.key = (note.key + key_or_param_id_delta).clamp(0, 127);
                            note.velocity =
                                (note.velocity + velocity_or_value_delta as f64).clamp(0.0, 127.0);
                            note.delay = (note.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
                            note.off = off;
                        }
                        LaneItem::Point(point) => {
                            point.module_index = point
                                .module_index
                                .saturating_add_signed(module_delta as isize);
                            point.param_id = point
                                .param_id
                                .saturating_add_signed(key_or_param_id_delta as i32);
                            point.value =
                                (point.value as i16 + velocity_or_value_delta).clamp(0, 0xff) as u8;
                            point.delay = (point.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
                        }
                    }
                    lane_item
                } else {
                    if module_delta != 0 {
                        LaneItem::Point(Point::default())
                    } else {
                        self.lane_item_last.clone()
                    }
                };

            self.sender_to_singer
                .send(SingerCommand::LaneItem(
                    self.cursor_track.clone(),
                    lane_item.clone(),
                ))
                .unwrap();

            if let LaneItem::Note(Note { off: false, .. }) = lane_item {
                self.lane_item_last = lane_item;
            }
        }

        Ok(())
    }

    pub fn run_track_command(&mut self, command: &TrackCommand) -> Result<()> {
        match command {
            TrackCommand::Copy => self.track_copy()?,
            TrackCommand::CursorLeft => self.track_prev(),
            TrackCommand::CursorRight => self.track_next(),
            TrackCommand::Cut => self.track_cut()?,
            TrackCommand::Delete => self.track_delete()?,
            TrackCommand::Dup => {}
            TrackCommand::MoveLeft => {}
            TrackCommand::MoveRight => {}
            TrackCommand::Paste => self.track_paste()?,
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
                self.sender_to_singer.send(SingerCommand::SongFile(
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
                self.send_to_plugin(MainToPlugin::StateSave(track_index, module_index), callback)?;
            }
        }

        Ok(())
    }

    fn track_cut(&mut self) -> Result<()> {
        self.track_copy()?;
        self.track_delete()?;
        Ok(())
    }

    fn track_delete(&mut self) -> Result<()> {
        self.sender_to_singer
            .send(SingerCommand::TrackDelete(self.cursor_track.track))?;
        Ok(())
    }

    fn track_paste(&mut self) -> Result<()> {
        if let Ok(text) = Clipboard::new()?.get_text() {
            if let Ok(track) = serde_json::from_str::<Track>(&text) {
                self.sender_to_singer
                    .send(SingerCommand::TrackInsert(self.cursor_track.track, track))?;
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
}

#[derive(Debug)]
pub enum AppStateCommand {
    Song(Song),
    Quit,
}

fn song_directory() -> PathBuf {
    let exe_path = current_exe().unwrap();
    let dir = exe_path.parent().unwrap();
    let dir = dir.join("user").join("song");
    create_dir_all(&dir).unwrap();
    dir
}
