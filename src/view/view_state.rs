use std::sync::mpsc::Sender;

use crate::{clap_manager::ClapManager, singer::SingerMsg};

use super::main_view::Route;

pub struct ViewState {
    pub view_sender: Sender<SingerMsg>,
    pub route: Route,
    pub plugin_selected: Option<String>,
    pub clap_manager: ClapManager,
}

impl ViewState {
    pub fn new(view_sender: Sender<SingerMsg>) -> Self {
        Self {
            view_sender,
            route: Route::Main,
            plugin_selected: None,
            clap_manager: ClapManager::new(),
        }
    }
}
