use std::{
    env::current_exe,
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
    time::{SystemTime, UNIX_EPOCH},
};

use arboard::Clipboard;
use common::{
    dsp::{db_from_norm, db_to_norm},
    module::Module,
    protocol::{MainToPlugin, PluginToMain},
    shmem::{open_shared_memory, SONG_STATE_NAME},
};
use eframe::egui::Color32;
use rfd::FileDialog;
use shared_memory::Shmem;

use crate::{
    command::{track_add::TrackAdd, Command},
    model::{note::Note, song::Song, track::Track},
    singer::SingerCommand,
    song_state::SongState,
    view::{
        root_view::Route,
        stereo_peak_meter::{DB_MAX, DB_MIN},
    },
};

pub enum UiCommand {
    Command,
    Follow,
    Loop,
    Mixer(MixerCommand),
    Module(ModuleCommand),
    NextViewPart,
    PlayToggle,
    SongSave,
    Track(TrackCommand),
    TrackAdd,
    TrackMute(Option<usize>, Option<bool>),
    TrackSolo(Option<usize>, Option<bool>),
    TrackPan(usize, f32),
    TrackVolume(usize, f32),
    LaneAdd,
}

pub enum TrackCommand {
    Copy,
    Cut,
    CursorDown,
    CursorLeft,
    CursorRight,
    CursorUp,
    NoteDelte,
    NoteUpdate(i16, i16, i16, bool),
    Paste,
    SelectMode,
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

    pub fn up(&mut self, _song: &Song) {
        if self.line != 0 {
            self.line -= 1;
        }
    }

    pub fn down(&mut self, _song: &Song) {
        self.line += 1;
    }

    pub fn left(&mut self, song: &Song) {
        if self.lane == 0 {
            if self.track == 0 {
                self.track = song.tracks.len() - 1;
            } else {
                self.track -= 1;
            }
            self.lane = song.tracks[self.track].lanes.len() - 1;
        } else {
            self.lane -= 1;
        }
    }

    pub fn right(&mut self, song: &Song) {
        if self.lane == song.tracks[self.track].lanes.len() - 1 {
            self.lane = 0;
            if self.track + 1 == song.tracks.len() {
                self.track = 0;
            } else {
                self.track += 1;
            }
        } else {
            self.lane += 1;
        }
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
    Module,
    Mixer,
}

pub struct AppState<'a> {
    pub hwnd: isize,
    pub focused_part: FocusedPart,
    pub follow_p: bool,
    pub cursor_track: CursorTrack,
    pub cursor_module: CursorModule,
    pub note_last: Note,
    pub route: Route,
    pub select_p: bool,
    pub selection_track_min: CursorTrack,
    pub selection_track_max: CursorTrack,
    pub selected_tracks: Vec<usize>,
    pub song: Song,
    pub song_dirty_p: bool,
    pub view_sender: Sender<SingerCommand>,
    pub sender_to_loop: Sender<MainToPlugin>,
    receiver_communicator_to_main_thread: Receiver<PluginToMain>,
    receiver_from_singer: Receiver<AppStateCommand>,
    nmodules_saving: usize,
    song_open_p: bool,
    _song_state_shmem: Shmem,
    pub song_state: &'a SongState,
    pub gui_context: Option<eframe::egui::Context>,
}

impl<'a> AppState<'a> {
    pub fn new(
        view_sender: Sender<SingerCommand>,
        sender_to_loop: Sender<MainToPlugin>,
        receiver_communicator_to_main_thread: Receiver<PluginToMain>,
        receiver_from_singer: Receiver<AppStateCommand>,
    ) -> Self {
        let song_state_shmem = open_shared_memory::<SongState>(SONG_STATE_NAME).unwrap();
        let song_state = unsafe { &*(song_state_shmem.as_ptr() as *const SongState) };

        Self {
            hwnd: 0,
            focused_part: FocusedPart::Track,
            follow_p: true,
            cursor_track: CursorTrack {
                track: 0,
                lane: 0,
                line: 0,
            },
            cursor_module: CursorModule { index: 0 },
            note_last: Note {
                delay: 0,
                channel: 0,
                key: 60,
                velocity: 100.0,
                off: false,
            },
            route: Route::Track,
            select_p: false,
            selection_track_min: Default::default(),
            selection_track_max: Default::default(),
            selected_tracks: vec![0],
            song: Song::new(),
            song_dirty_p: false,
            view_sender,
            sender_to_loop,
            receiver_communicator_to_main_thread,
            receiver_from_singer,
            nmodules_saving: 0,
            song_open_p: false,
            _song_state_shmem: song_state_shmem,
            song_state,
            gui_context: None,
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
            Color32::YELLOW
        } else {
            Color32::from_rgb(200, 200, 0)
        }
    }

    pub fn cursor_up(&mut self) {
        self.cursor_track.up(&self.song);
    }

    pub fn cursor_down(&mut self) {
        self.cursor_track.down(&self.song);
    }

    pub fn cursor_left(&mut self) {
        self.cursor_track.left(&self.song);
        self.selected_tracks.clear();
        self.selected_tracks.push(self.cursor_track.track);
    }

    pub fn cursor_right(&mut self) {
        self.cursor_track.right(&self.song);
        self.selected_tracks.clear();
        self.selected_tracks.push(self.cursor_track.track);
    }

    pub fn module_mut(&mut self, track_index: usize, module_index: usize) -> Option<&mut Module> {
        self.song
            .tracks
            .get_mut(track_index)
            .and_then(|x| x.modules.get_mut(module_index))
    }

    pub fn receive_from_singer(&mut self) -> anyhow::Result<()> {
        while let Ok(command) = self.receiver_from_singer.try_recv() {
            match command {
                AppStateCommand::Song(song) => {
                    if self.song_open_p {
                        self.song_open_did(song).unwrap();
                        self.song_open_p = false;
                        self.song_dirty_p = false;
                        dbg!(1, self.song_dirty_p);
                    } else {
                        self.song = song;
                        self.song_dirty_p = true;
                        dbg!(2, self.song_dirty_p);
                    }
                }
                AppStateCommand::Quit => (),
            }
        }

        Ok(())
    }

    pub fn receive_from_communicator(&mut self) -> anyhow::Result<()> {
        while let Ok(message) = self.receiver_communicator_to_main_thread.try_recv() {
            match message {
                PluginToMain::DidLoad => (),
                PluginToMain::DidUnload(_track_index, _module_index) => (),
                PluginToMain::DidGuiOpen => (),
                PluginToMain::DidScan => {}
                PluginToMain::DidStateLoad => (),
                PluginToMain::DidStateSave(track_index, module_index, state) => {
                    if let Some(module) = self.module_mut(track_index, module_index) {
                        module.state = Some(state);
                    }
                    if self.nmodules_saving > 0 {
                        self.nmodules_saving -= 1;
                        if self.nmodules_saving == 0 {
                            self.song_save_file()?;
                        }
                    }
                }
                PluginToMain::Quit => (),
            }
        }
        Ok(())
    }

    pub fn run_ui_command(&mut self, command: &UiCommand) -> anyhow::Result<()> {
        match command {
            UiCommand::Command => {
                self.route = Route::Command;
            }
            UiCommand::Follow => {
                self.follow_p = !self.follow_p;
            }
            UiCommand::Loop => {
                self.view_sender.send(SingerCommand::Loop)?;
            }
            UiCommand::NextViewPart => {
                self.focused_part = match self.focused_part {
                    FocusedPart::Track => FocusedPart::Module,
                    FocusedPart::Module => FocusedPart::Mixer,
                    FocusedPart::Mixer => FocusedPart::Track,
                }
            }
            UiCommand::PlayToggle => {
                if self.song_state.play_p {
                    self.view_sender.send(SingerCommand::Stop)?;
                } else {
                    self.view_sender.send(SingerCommand::Play)?;
                }
            }
            UiCommand::SongSave => {
                self.song_save()?;
            }
            UiCommand::TrackAdd => {
                TrackAdd {}.call(self)?;
            }
            UiCommand::TrackMute(track_index, mute) => {
                let track_index = track_index.unwrap_or(self.cursor_track.track);
                let mute = mute.unwrap_or(!self.song.tracks[track_index].mute);
                self.view_sender
                    .send(SingerCommand::TrackMute(track_index, mute))?;
            }
            UiCommand::TrackSolo(track_index, solo) => {
                let track_index = track_index.unwrap_or(self.cursor_track.track);
                let solo = solo.unwrap_or(!self.song.tracks[track_index].solo);
                self.view_sender
                    .send(SingerCommand::TrackSolo(track_index, solo))?;
            }
            UiCommand::TrackPan(track_index, pan) => self
                .view_sender
                .send(SingerCommand::TrackPan(*track_index, *pan))?,
            UiCommand::TrackVolume(track_index, volume) => self
                .view_sender
                .send(SingerCommand::TrackVolume(*track_index, *volume))?,
            UiCommand::LaneAdd => self
                .view_sender
                .send(SingerCommand::LaneAdd(self.cursor_track.track))?,
            UiCommand::Track(TrackCommand::Copy) => self.notes_copy()?,
            UiCommand::Track(TrackCommand::Cut) => self.notes_cut()?,
            UiCommand::Track(TrackCommand::Paste) => self.notes_paste()?,
            UiCommand::Track(TrackCommand::CursorUp) => self.cursor_up(),
            UiCommand::Track(TrackCommand::CursorDown) => self.cursor_down(),
            UiCommand::Track(TrackCommand::CursorLeft) => self.cursor_left(),
            UiCommand::Track(TrackCommand::CursorRight) => self.cursor_right(),
            UiCommand::Track(TrackCommand::NoteDelte) => self
                .view_sender
                .send(SingerCommand::NoteDelete(self.cursor_track.clone()))?,
            UiCommand::Track(TrackCommand::NoteUpdate(
                key_delta,
                velociy_delta,
                delay_delta,
                off,
            )) => {
                note_update(*key_delta, *velociy_delta, *delay_delta, *off, self);
            }
            UiCommand::Track(TrackCommand::SelectMode) => {
                self.select_p = !self.select_p;
                if self.select_p {
                    self.selection_track_min = self.cursor_track;
                    self.selection_track_max = self.cursor_track;
                } else {
                    self.selection_track_min =
                        self.selection_track_min.min_merge(&self.cursor_track);
                    self.selection_track_max =
                        self.selection_track_max.max_merge(&self.cursor_track);
                }
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
                    self.view_sender.send(SingerCommand::TrackPan(
                        self.cursor_track.track,
                        (track.pan + (delta / 20.0)).clamp(0.0, 1.0),
                    ))?;
                }
            }
            UiCommand::Mixer(MixerCommand::Volume(delta)) => {
                if let Some(track) = self.track_current() {
                    let db = db_from_norm(track.volume, DB_MIN, DB_MAX) + delta;
                    self.view_sender.send(SingerCommand::TrackVolume(
                        self.cursor_track.track,
                        db_to_norm(db, DB_MIN, DB_MAX).clamp(0.0, 1.0),
                    ))?;
                }
            }
        }
        Ok(())
    }

    pub fn song_open(&mut self) -> anyhow::Result<()> {
        if let Some(path) = FileDialog::new()
            .set_directory(song_directory())
            .pick_file()
        {
            self.song_open_p = true;
            self.view_sender.send(SingerCommand::SongOpen(
                path.to_str().map(|s| s.to_string()).unwrap(),
                self.hwnd,
            ))?;
        }
        Ok(())
    }

    pub fn song_open_did(&mut self, song: Song) -> anyhow::Result<()> {
        self.song = song;
        Ok(())
    }

    pub fn song_save(&mut self) -> anyhow::Result<()> {
        self.nmodules_saving = 0;
        for track_index in 0..self.song.tracks.len() {
            for module_index in 0..self.song.tracks[track_index].modules.len() {
                self.sender_to_loop
                    .send(MainToPlugin::StateSave(track_index, module_index))?;
                self.nmodules_saving += 1;
            }
        }
        if self.nmodules_saving == 0 {
            self.song_save_file()?;
        }
        Ok(())
    }

    fn notes_copy(&mut self) -> anyhow::Result<()> {
        self.notes_copy_or_cut(true)
    }

    fn notes_copy_or_cut(&mut self, copy_p: bool) -> anyhow::Result<()> {
        if self.select_p {
            self.run_ui_command(&UiCommand::Track(TrackCommand::SelectMode))?;
        }
        let min = &self.selection_track_min;
        let max = &self.selection_track_max;
        let mut notess = vec![];
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
                        let mut notes = vec![];
                        for line in min.line..=max.line {
                            notes.push(lane.note(line).clone());
                            if !copy_p {
                                self.view_sender
                                    .send(SingerCommand::NoteDelete(CursorTrack {
                                        track: track_index,
                                        lane: lane_index,
                                        line,
                                    }))?;
                            }
                        }
                        notess.push(notes);
                    }
                }
            }
        }

        let json = serde_json::to_string_pretty(&notess)?;
        self.gui_context.as_ref().unwrap().copy_text(json);

        Ok(())
    }

    fn notes_cut(&mut self) -> anyhow::Result<()> {
        self.notes_copy_or_cut(false)
    }

    fn notes_paste(&mut self) -> anyhow::Result<()> {
        let mut clipboard = Clipboard::new().unwrap();
        if let Ok(text) = clipboard.get_text() {
            let mut cursor = self.cursor_track.clone();
            if let Ok(notess) = serde_json::from_str::<Vec<Vec<Option<Note>>>>(&text) {
                for notes in notess.into_iter() {
                    for note in notes.into_iter() {
                        if let Some(note) = note {
                            self.view_sender.send(SingerCommand::Note(cursor, note))?;
                        } else {
                            self.view_sender.send(SingerCommand::NoteDelete(cursor))?;
                        }
                        cursor.down(&self.song);
                    }
                    cursor.right(&self.song);
                    cursor.line = self.cursor_track.line;
                }
            }
        }

        Ok(())
    }

    fn song_save_file(&mut self) -> anyhow::Result<()> {
        let song_file = if let Some(song_file) = &self.song_state.song_file_get() {
            song_file.into()
        } else {
            if let Some(path) = FileDialog::new()
                .set_directory(song_directory())
                .set_file_name(&self.song.name)
                .save_file()
            {
                self.view_sender.send(SingerCommand::SongFile(
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

fn note_update(
    key_delta: i16,
    velociy_delta: i16,
    delay_delta: i16,
    off: bool,
    state: &mut AppState,
) {
    if let Some(note) = state.song.note(&state.cursor_track) {
        if !note.off {
            let mut note = note.clone();
            note.key = (note.key + key_delta).clamp(0, 127);
            note.velocity = (note.velocity + velociy_delta as f64).clamp(0.0, 127.0);
            note.delay = (note.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
            state.note_last = note;
        }
    }

    let mut note = state.note_last.clone();
    note.off = off;
    state
        .view_sender
        .send(SingerCommand::Note(state.cursor_track.clone(), note))
        .unwrap();
}
