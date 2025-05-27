use serde::{Deserialize, Serialize};

use crate::note::Note;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Track {
    notes: Vec<Note>,
}

impl Track {
    pub fn new() -> Self {
        Self { notes: vec![] }
    }
}
