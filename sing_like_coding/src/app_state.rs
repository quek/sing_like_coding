use std::sync::mpsc::Sender;

use common::{
    clap_manager::ClapManager,
    protocol::{MainToPlugin, PluginToMain},
};
use eframe::egui;

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
}

impl AppState {
    pub fn new(view_sender: Sender<SingerCommand>, sender_to_loop: Sender<MainToPlugin>) -> Self {
        Self {
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
            },
            route: Route::Track,
            selected_cells: vec![(0, 0)],
            selected_tracks: vec![0],
            song: Song::new(),
            song_state: SongState::default(),
            view_sender,
            sender_to_loop,
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

    pub fn received_from_plugin_process(&mut self, message: PluginToMain) -> anyhow::Result<()> {
        match message {
            PluginToMain::DidLoad => (),
            PluginToMain::DidGuiOpen => (),
            PluginToMain::DidScan => {
                self.clap_manager.load()?;
            }
            PluginToMain::Quit => (),
        }
        Ok(())
    }
}
