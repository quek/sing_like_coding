use std::{
    env::current_exe,
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use common::{
    clap_manager::ClapManager,
    module::Module,
    protocol::{MainToPlugin, PluginToMain},
};
use eframe::egui;
use rfd::FileDialog;

use crate::{
    model::{note::Note, song::Song},
    singer::{SingerCommand, SongState},
    view::main_view::Route,
};

#[derive(Clone, Debug)]
pub struct Cursor {
    pub track: usize,
    pub lane: usize,
    pub line: usize,
}

pub struct AppState {
    pub hwnd: isize,
    pub gui_context: Option<egui::Context>,
    pub clap_manager: ClapManager,
    pub cursor: Cursor,
    pub note_last: Note,
    pub route: Route,
    pub selected_cells: Vec<(usize, usize)>,
    pub selected_tracks: Vec<usize>,
    pub song: Song,
    pub song_state: SongState,
    pub view_sender: Sender<SingerCommand>,
    pub sender_to_loop: Sender<MainToPlugin>,
    nmodules_saving: usize,
    pub song_open_p: bool,
}

impl AppState {
    pub fn new(view_sender: Sender<SingerCommand>, sender_to_loop: Sender<MainToPlugin>) -> Self {
        Self {
            hwnd: 0,
            gui_context: None,
            clap_manager: ClapManager::new(),
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
            song_state: SongState::default(),
            view_sender,
            sender_to_loop,
            nmodules_saving: 0,
            song_open_p: false,
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

    pub fn received_from_plugin_process(&mut self, message: PluginToMain) -> anyhow::Result<()> {
        match message {
            PluginToMain::DidLoad => (),
            PluginToMain::DidUnload(_track_index, _module_index) => (),
            PluginToMain::DidGuiOpen => (),
            PluginToMain::DidScan => {
                self.clap_manager.load()?;
            }
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
        let song_file = if let Some(song_file) = &self.song_state.song_file {
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
    State(SongState),
    Quit,
}

pub fn loop_receive_from_audio_thread(
    state: Arc<Mutex<AppState>>,
    receiver: Receiver<AppStateCommand>,
    gui_context: &eframe::egui::Context,
) {
    let gui_context = gui_context.clone();
    thread::spawn(move || {
        while let Ok(command) = receiver.recv() {
            match command {
                AppStateCommand::Song(song) => {
                    let mut state = state.lock().unwrap();
                    if state.song_open_p {
                        state.song_open_did(song).unwrap();
                    } else {
                        state.song = song;
                    }
                    gui_context.request_repaint();
                }
                AppStateCommand::State(song_state) => {
                    state.lock().unwrap().song_state = song_state;
                    gui_context.request_repaint();
                }
                AppStateCommand::Quit => return,
            }
        }
    });
}

fn song_directory() -> PathBuf {
    let exe_path = current_exe().unwrap();
    let dir = exe_path.parent().unwrap();
    dir.join("user").join("song")
}
