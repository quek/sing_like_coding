use std::{
    env::current_exe,
    fs::{create_dir_all, File},
    io::Write,
    path::PathBuf,
    sync::mpsc::{Receiver, Sender},
};

use common::{
    module::Module,
    protocol::{MainToPlugin, PluginToMain},
    shmem::{open_shared_memory, SONG_STATE_NAME},
};
use rfd::FileDialog;
use shared_memory::Shmem;

use crate::{
    command::{track_add::TrackAdd, Command},
    model::{note::Note, song::Song},
    singer::SingerCommand,
    song_state::SongState,
    view::root_view::Route,
};

pub enum UiCommand {
    Command,
    NextViewPart,
    NoteUpdate(i16, i16, i16, bool),
    NoteDelte,
    PlayToggle,
    TrackAdd,
    TrackMute(usize, bool),
    TrackSolo(usize, bool),
    TrackPan(usize, f32),
    TrackVolume(usize, f32),
    LaneAdd,
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
}

#[derive(Clone, Debug)]
pub struct Cursor {
    pub track: usize,
    pub lane: usize,
    pub line: usize,
}

pub enum FocusedPart {
    Track,
    Module,
    Mixer,
}

pub struct AppState<'a> {
    pub hwnd: isize,
    pub focused_part: FocusedPart,
    pub cursor: Cursor,
    pub note_last: Note,
    pub route: Route,
    pub selected_cells: Vec<(usize, usize)>,
    pub selected_tracks: Vec<usize>,
    pub song: Song,
    pub view_sender: Sender<SingerCommand>,
    pub sender_to_loop: Sender<MainToPlugin>,
    receiver_communicator_to_main_thread: Receiver<PluginToMain>,
    receiver_from_singer: Receiver<AppStateCommand>,
    nmodules_saving: usize,
    pub song_open_p: bool,
    _song_state_shmem: Shmem,
    pub song_state: &'a SongState,
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
            cursor: Cursor {
                track: 0,
                lane: 0,
                line: 0,
            },
            note_last: Note {
                line: 0,
                delay: 0,
                channel: 0,
                key: 60,
                velocity: 100.0,
                off: false,
            },
            route: Route::Track,
            selected_cells: vec![(0, 0)],
            selected_tracks: vec![0],
            song: Song::new(),
            view_sender,
            sender_to_loop,
            receiver_communicator_to_main_thread,
            receiver_from_singer,
            nmodules_saving: 0,
            song_open_p: false,
            _song_state_shmem: song_state_shmem,
            song_state,
        }
    }

    pub fn cursor_up(&mut self) {
        if self.cursor.line != 0 {
            self.cursor.line -= 1;
        }
    }

    pub fn cursor_down(&mut self) {
        self.cursor.line += 1;
    }

    pub fn cursor_left(&mut self) {
        if self.cursor.lane == 0 {
            if self.cursor.track == 0 {
                self.cursor.track = self.song.tracks.len() - 1;
            } else {
                self.cursor.track -= 1;
            }
            self.cursor.lane = self.song.tracks[self.cursor.track].lanes.len() - 1;
        } else {
            self.cursor.lane -= 1;
        }
        self.selected_tracks.clear();
        self.selected_tracks.push(self.cursor.track);
    }

    pub fn cursor_right(&mut self) {
        if self.cursor.lane == self.song.tracks[self.cursor.track].lanes.len() - 1 {
            self.cursor.lane = 0;
            if self.cursor.track + 1 == self.song.tracks.len() {
                self.cursor.track = 0;
            } else {
                self.cursor.track += 1;
            }
        } else {
            self.cursor.lane += 1;
        }
        self.selected_tracks.clear();
        self.selected_tracks.push(self.cursor.track);
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
                    } else {
                        self.song = song;
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
            UiCommand::NextViewPart => {
                self.focused_part = match self.focused_part {
                    FocusedPart::Track => FocusedPart::Module,
                    FocusedPart::Module => FocusedPart::Mixer,
                    FocusedPart::Mixer => FocusedPart::Track,
                }
            }
            UiCommand::NoteUpdate(key_delta, velociy_delta, delay_delta, off) => {
                note_update(*key_delta, *velociy_delta, *delay_delta, *off, self);
            }
            UiCommand::NoteDelte => self
                .view_sender
                .send(SingerCommand::NoteDelete(self.cursor.clone()))?,
            UiCommand::PlayToggle => {
                if self.song_state.play_p {
                    self.view_sender.send(SingerCommand::Stop)?;
                } else {
                    self.view_sender.send(SingerCommand::Play)?;
                }
            }
            UiCommand::TrackAdd => {
                TrackAdd {}.call(self)?;
            }
            UiCommand::TrackMute(track_index, mute) => self
                .view_sender
                .send(SingerCommand::TrackMute(*track_index, *mute))?,
            UiCommand::TrackSolo(track_index, solo) => self
                .view_sender
                .send(SingerCommand::TrackSolo(*track_index, *solo))?,
            UiCommand::TrackPan(track_index, pan) => self
                .view_sender
                .send(SingerCommand::TrackPan(*track_index, *pan))?,
            UiCommand::TrackVolume(track_index, volume) => self
                .view_sender
                .send(SingerCommand::TrackVolume(*track_index, *volume))?,
            UiCommand::LaneAdd => self
                .view_sender
                .send(SingerCommand::LaneAdd(self.cursor.track))?,
            UiCommand::CursorUp => self.cursor_up(),
            UiCommand::CursorDown => self.cursor_down(),
            UiCommand::CursorLeft => self.cursor_left(),
            UiCommand::CursorRight => self.cursor_right(),
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
        Ok(())
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
    if let Some(note) = state.song.note(&state.cursor) {
        if !note.off {
            let mut note = note.clone();
            note.key = (note.key + key_delta).clamp(0, 127);
            note.velocity = (note.velocity + velociy_delta as f64).clamp(0.0, 127.0);
            note.delay = (note.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
            state.note_last = note;
        }
    }

    let mut note = state.note_last.clone();
    note.line = state.cursor.line;
    note.off = off;
    state
        .view_sender
        .send(SingerCommand::Note(state.cursor.clone(), note))
        .unwrap();
}
