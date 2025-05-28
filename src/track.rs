use std::pin::Pin;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{
    event_list::{EventListInput, EventListOutput},
    note::Note,
    plugin::Plugin,
    song::{self},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub name: String,
    pub nlines: usize,
}

impl State {
    pub fn new() -> Self {
        Self {
            name: "T01".to_string(),
            nlines: 16,
        }
    }
}

pub struct Track {
    pub state: State,
    pub notes: Vec<Note>,
    pub modules: Vec<Pin<Box<Plugin>>>,
    pub event_list_input: Pin<Box<EventListInput>>,
    event_list_output: Pin<Box<EventListOutput>>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            state: State::new(),
            notes: vec![],
            modules: vec![],
            event_list_input: EventListInput::new(),
            event_list_output: EventListOutput::new(),
        }
    }

    pub fn note(&self, line: usize) -> Option<&Note> {
        self.notes.iter().find(|note| note.line == line)
    }

    pub fn note_mut(&mut self, line: usize) -> Option<&mut Note> {
        self.notes.iter_mut().find(|note| note.line == line)
    }

    pub fn process(
        &mut self,
        state: &song::State,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        if let Some(module) = self.modules.first_mut() {
            module.process(
                state,
                buffer,
                frames_count,
                steady_time,
                &mut self.event_list_input,
                &mut self.event_list_output,
            )?;
        }

        self.event_list_input.clear();
        // TODO プラグインからの MIDI イベントの処理
        self.event_list_output.clear();

        Ok(())
    }
}
