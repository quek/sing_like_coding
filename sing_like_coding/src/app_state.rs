use std::sync::mpsc::Sender;

use common::{
    clap_manager::ClapManager,
    protocol::{MainToPlugin, PluginToMain},
};
use eframe::egui;

use crate::{
    model::song::Song,
    singer::{ClapPluginPtr, SingerMsg, SongState},
    view::main_view::Route,
};

pub struct AppState {
    pub gui_context: Option<egui::Context>,
    pub clap_manager: ClapManager,
    pub cursor_line: usize,
    pub cursor_track: usize,
    pub key_last: i16,
    pub line_buffers: Vec<Vec<String>>,
    pub route: Route,
    pub selected_cells: Vec<(usize, usize)>,
    pub selected_tracks: Vec<usize>,
    pub song: Song,
    pub song_state: SongState,
    pub view_sender: Sender<SingerMsg>,
    pub sender_to_loop: Sender<MainToPlugin>,
    pub callback_plugins: Vec<ClapPluginPtr>,
}

impl AppState {
    pub fn new(view_sender: Sender<SingerMsg>, sender_to_loop: Sender<MainToPlugin>) -> Self {
        Self {
            gui_context: None,
            clap_manager: ClapManager::new(),
            cursor_line: 0,
            cursor_track: 0,
            key_last: 60,
            line_buffers: vec![],
            route: Route::Track,
            selected_cells: vec![(0, 0)],
            selected_tracks: vec![0],
            song: Song::new(),
            song_state: SongState::default(),
            view_sender,
            sender_to_loop,
            callback_plugins: vec![],
        }
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
