use std::sync::{Arc, Mutex};

use anyhow::Result;
use common::{
    event::Event,
    module::Module,
    process_track_context::ProcessTrackContext,
    protocol::{receive, send, AudioToPlugin, PluginToAudio},
};
use serde::{Deserialize, Serialize};

use crate::model::note::Note;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub modules: Vec<Module>,
    pub notes: Vec<Note>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            name: "T01".to_string(),
            modules: vec![],
            notes: vec![],
        }
    }

    pub fn compute_midi(&self, context: &mut ProcessTrackContext) {
        for note in self.notes.iter() {
            let time = note.line * 0x100 + note.delay as usize;
            if context.play_position.contains(&(time as i64)) {
                if let Some(key) = context.on_key {
                    context.event_list_input.push(Event::NoteOff(key));
                }
                // TODO time
                context
                    .event_list_input
                    .push(Event::NoteOn(note.key, note.velocity));
                context.on_key = Some(note.key);
            }
        }
    }

    pub async fn process(&self, context: Arc<Mutex<ProcessTrackContext>>) -> Result<()> {
        let mut context = context.lock().unwrap();
        self.compute_midi(&mut context);
        let module_len = self.modules.len();
        for module_index in 0..module_len {
            self.process_module(&mut context, module_index).await?;
        }

        Ok(())
    }

    async fn process_module(
        &self,
        context: &mut ProcessTrackContext,
        module_index: usize,
    ) -> Result<()> {
        let pipe = &mut context.plugins[module_index].pipe;
        let message = AudioToPlugin::Process(context.buffer.clone());
        // log::debug!("#### will send to plugin audio thread {:?}", message);
        // send(pipe, &message).await?;
        let res = send(pipe, &message).await;
        // log::debug!("#### send result: {:?}", res);
        // log::debug!("#### did send");
        let message: PluginToAudio = receive(pipe).await?;
        // log::debug!("#### received from plugin audio thread {:?}", message);
        match message {
            PluginToAudio::Process(audio_buffer) => {
                context.buffer = audio_buffer;
            }
            PluginToAudio::B => (),
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub fn note(&self, line: usize) -> Option<&Note> {
        self.notes.iter().find(|note| note.line == line)
    }

    #[allow(dead_code)]
    pub fn note_mut(&mut self, line: usize) -> Option<&mut Note> {
        self.notes.iter_mut().find(|note| note.line == line)
    }
}
