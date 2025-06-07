use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::note::Note;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lane {
    pub notes: HashMap<usize, Note>,
}

impl Lane {
    pub fn new() -> Self {
        Self {
            notes: Default::default(),
        }
    }

    pub fn note(&self, line: usize) -> Option<&Note> {
        self.notes.get(&line)
    }

    #[allow(dead_code)]
    pub fn note_mut(&mut self, line: usize) -> Option<&mut Note> {
        self.notes.get_mut(&line)
    }
}
