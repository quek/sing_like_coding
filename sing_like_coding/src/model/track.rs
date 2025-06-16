use anyhow::Result;
use common::{
    dsp::db_to_norm, event::Event, module::Module, process_track_context::ProcessTrackContext,
};
use serde::{Deserialize, Serialize};

use super::{lane::Lane, lane_item::LaneItem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    #[serde(default)]
    pub volume: f32,
    #[serde(default)]
    pub pan: f32,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub solo: bool,
    pub modules: Vec<Module>,
    pub lanes: Vec<Lane>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            name: "T01".to_string(),
            volume: db_to_norm(0.0, -60.0, 6.0),
            pan: 0.5,
            solo: false,
            mute: false,
            modules: vec![],
            lanes: vec![Lane::new()],
        }
    }

    pub fn compute_midi(&self, context: &mut ProcessTrackContext) {
        if !context.play_p {
            return;
        }
        let ranges = if context.play_position.start < context.play_position.end {
            vec![context.play_position.clone()]
        } else {
            vec![
                context.play_position.start..context.loop_range.end,
                context.loop_range.start..context.play_position.end,
            ]
        };
        for range in ranges {
            let line_start = range.start / 0x100;
            let line_end = range.end / 0x100;
            for line in line_start..=line_end {
                for (lane_index, lane) in self.lanes.iter().enumerate() {
                    if let Some((line, item)) = lane.items.get_key_value(&line) {
                        let time = *line * 0x100 + item.delay() as usize;
                        match item {
                            LaneItem::Note(note) => {
                                if range.contains(&time) {
                                    let delay = time - range.start;
                                    if let Some(Some(key)) = context.on_keys.get(lane_index).take()
                                    {
                                        context.event_list_input.push(Event::NoteOff(*key, delay));
                                    }
                                    if !note.off {
                                        context.event_list_input.push(Event::NoteOn(
                                            note.key,
                                            note.velocity,
                                            delay,
                                        ));
                                        if context.on_keys.len() <= lane_index {
                                            context.on_keys.resize_with(lane_index + 1, || None);
                                        }
                                        context.on_keys[lane_index] = Some(note.key);
                                    }
                                }
                            }
                            LaneItem::Point(point) => {
                                if range.contains(&time) {
                                    let delay = time - range.start;
                                    context.event_list_input.push(Event::ParamValue(
                                        point.param_id,
                                        point.value as f64 / 255.0,
                                        delay,
                                    ))
                                }
                            }
                        }
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
        let data = context.plugins[module_index].process_data_mut();
        for event in context.event_list_input.drain(..) {
            match event {
                Event::NoteOn(key, velocity, delay) => data.note_on(key, velocity, 0, delay),
                Event::NoteOff(key, delay) => data.note_off(key, 0, delay),
                Event::NoteAllOff => {
                    for key in context.on_keys.drain(..).filter_map(|x| x) {
                        data.note_off(key, 0, 0);
                    }
                }
                Event::ParamValue(param_id, value, delay) => {
                    data.param_value(param_id, value, delay)
                }
            }
        }

        if module_index > 0 {
            let (left, right) = context.plugins.split_at_mut(module_index);
            let prev = &mut left[module_index - 1];
            let curr = &mut right[0];

            let constant_mask = prev.process_data_mut().constant_mask_out;
            curr.process_data_mut().constant_mask_in = constant_mask;

            let buffer_out = &prev.process_data_mut().buffer_out;
            let buffer_in = &mut curr.process_data_mut().buffer_in;

            for ch in 0..context.nchannels {
                if (constant_mask & (1 << ch)) == 0 {
                    buffer_in[ch].copy_from_slice(&buffer_out[ch]);
                } else {
                    buffer_in[ch][0] = buffer_out[ch][0];
                }
            }
        }

        context.plugins[module_index].process()?;

        Ok(())
    }
}
