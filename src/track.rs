use std::pin::Pin;

use anyhow::Result;

use crate::{note::Note, plugin::Plugin};

pub struct Track {
    pub notes: Vec<Note>,
    pub modules: Vec<Pin<Box<Plugin>>>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            notes: vec![],
            modules: vec![],
        }
    }

    pub fn process(
        &mut self,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        if let Some(module) = self.modules.first_mut() {
            module.process(buffer, frames_count, steady_time)?;
        }
        Ok(())
    }
}
