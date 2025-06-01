use std::sync::mpsc::Sender;

use crate::{
    clap_manager::ClapManager,
    model::song::Song,
    singer::{SingerMsg, SongState},
};

use super::main_view::Route;

pub struct ViewState {
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
}

impl ViewState {
    pub fn new(view_sender: Sender<SingerMsg>) -> Self {
        Self {
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
        }
    }
}
