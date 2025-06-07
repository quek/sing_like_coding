use anyhow::Result;
use common::{event::Event, module::Module, process_track_context::ProcessTrackContext};
use serde::{Deserialize, Serialize};

use super::lane::Lane;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub modules: Vec<Module>,
    pub lanes: Vec<Lane>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            name: "T01".to_string(),
            modules: vec![],
            lanes: vec![Lane::new()],
        }
    }

    pub fn compute_midi(&self, context: &mut ProcessTrackContext) {
        let line_start = context.play_position.start / 0x100;
        let line_end = context.play_position.end / 0x100;
        for line in line_start..=line_end {
            for (lane_index, lane) in self.lanes.iter().enumerate() {
                if let Some(note) = lane.notes.get(&(line as usize)) {
                    let time = note.line * 0x100 + note.delay as usize;
                    if context.play_position.contains(&(time as i64)) {
                        if let Some(Some(key)) = context.on_keys.get(lane_index) {
                            context.event_list_input.push(Event::NoteOff(*key));
                        }
                        context
                            .event_list_input
                            .push(Event::NoteOn(note.key, note.velocity));
                        if context.on_keys.len() <= lane_index {
                            context.on_keys.resize_with(lane_index + 1, || None);
                        }
                        context.on_keys[lane_index] = Some(note.key);
                    }
                }
            }
        }
    }

    pub fn process(&self, context: &mut ProcessTrackContext) -> Result<()> {
        self.compute_midi(context);
        let module_len = self.modules.len();
        for module_index in 0..module_len {
            self.process_module(context, module_index)?;
        }

        Ok(())
    }

    fn process_module(&self, context: &mut ProcessTrackContext, module_index: usize) -> Result<()> {
        let data = context.plugins[module_index].process_data();
        for event in context.event_list_input.drain(..) {
            match event {
                Event::NoteOn(key, velocity) => data.note_on(key, velocity, 0, 0),
                Event::NoteOff(key) => data.note_off(key, 0, 0),
                Event::NoteAllOff => {
                    for key in context.on_keys.drain(..).filter_map(|x| x) {
                        data.note_off(key, 0, 0);
                    }
                }
            }
        }

        if module_index > 0 {
            let (left, right) = context.plugins.split_at_mut(module_index);
            let prev = &mut left[module_index - 1];
            let curr = &mut right[0];

            let buffer_out = &prev.process_data().buffer_out;
            let buffer_in = &mut curr.process_data().buffer_in;

            for ch in 0..context.nchannels {
                buffer_in[ch].copy_from_slice(&buffer_out[ch]);
            }
        }

        context.plugins[module_index].process()?;

        Ok(())
    }
}
