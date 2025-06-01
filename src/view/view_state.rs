use std::sync::mpsc::Sender;

use crate::{clap_manager::ClapManager, singer::SingerMsg};

use super::main_view::Route;

pub struct ViewState {
    pub view_sender: Sender<SingerMsg>,
    pub route: Route,
    pub plugin_selected: Option<String>,
    pub clap_manager: ClapManager,
    pub cursor_position: (usize, usize),
    pub selected_tracks: Vec<usize>,
    pub selected_cells: Vec<(usize, usize)>,
    pub line_buffers: Vec<Vec<String>>,
}

impl ViewState {
    pub fn new(view_sender: Sender<SingerMsg>) -> Self {
        Self {
            view_sender,
            route: Route::Track,
            plugin_selected: None,
            clap_manager: ClapManager::new(),
            cursor_position: (0, 0),
            selected_tracks: vec![0],
            selected_cells: vec![(0, 0)],
            line_buffers: vec![],
        }
    }
}
