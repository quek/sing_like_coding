use std::sync::{Arc, Mutex};

use anyhow::Result;
use clap_sys::id::clap_id;
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
    pub automation_params: Vec<(usize, clap_id)>, // (module_index, param_id)
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
            automation_params: vec![],
        }
    }

    pub fn process(
        &self,
        track_index: usize,
        contexts: &Vec<Arc<Mutex<ProcessTrackContext>>>,
    ) -> Result<()> {
        let mut context = contexts[track_index].lock().unwrap();
        self.compute_midi(&mut context);
        let module_len = self.modules.len();
        for module_index in 0..module_len {
            self.process_module(track_index, &mut context, module_index, contexts)?;
        }

        Ok(())
    }

    fn process_module(
        &self,
        track_index: usize,
        context: &mut ProcessTrackContext,
        module_index: usize,
        contexts: &Vec<Arc<Mutex<ProcessTrackContext>>>,
    ) -> Result<()> {
        self.prepare_module_event(context, module_index)?;

        for autdio_input in self.modules[module_index].audio_inputs.iter() {
            let src_ptr = if autdio_input.track_index == track_index {
                context.plugins[autdio_input.module_index].ptr
            } else {
                let context = contexts[autdio_input.track_index].lock().unwrap();
                context.plugins[module_index].ptr
            };
            let src_process_data = unsafe { &*src_ptr };
            let src_constant_mask = src_process_data.constant_mask_out;
            dbg!(src_process_data.constant_mask_out);
            let src_buffer = &src_process_data.buffer_out;
            let self_process_data = context.plugins[module_index].process_data_mut();
            let self_buffer = &mut self_process_data.buffer_in;

            for ch in 0..context.nchannels {
                let constant_mask_bit = 1 << ch;
                if (src_constant_mask & constant_mask_bit) == 0 {
                    self_process_data.constant_mask_in &= !constant_mask_bit;
                    self_buffer[ch].copy_from_slice(&src_buffer[ch]);
                } else {
                    self_process_data.constant_mask_in |= constant_mask_bit;
                    self_buffer[ch][0] = src_buffer[ch][0];
                }
            }
            log::debug!(
                "{} {} {} {}",
                track_index,
                module_index,
                src_constant_mask,
                self_process_data.constant_mask_in
            );
        }

        context.plugins[module_index].process()?;

        Ok(())
    }

    fn compute_midi(&self, context: &mut ProcessTrackContext) {
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
                                    let (module_index, param_id) =
                                        self.automation_params[point.automation_params_index];
                                    context.event_list_input.push(Event::ParamValue(
                                        module_index,
                                        param_id,
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

    fn prepare_module_event(
        &self,
        context: &mut ProcessTrackContext,
        module_index: usize,
    ) -> Result<()> {
        let plugin_ref_self = &mut context.plugins[module_index];
        let data = plugin_ref_self.process_data_mut();
        for event in context.event_list_input.iter() {
            match event {
                Event::NoteOn(key, velocity, delay) => {
                    data.input_note_on(*key, *velocity, 0, *delay)
                }
                Event::NoteOff(key, delay) => data.input_note_off(*key, 0, *delay),
                Event::NoteAllOff => {
                    for key in context.on_keys.drain(..).filter_map(|x| x) {
                        data.input_note_off(key, 0, 0);
                    }
                }
                Event::ParamValue(mindex, param_id, value, delay) => {
                    if *mindex == module_index {
                        data.input_param_value(*param_id, *value, *delay)
                    }
                }
            }
        }
        Ok(())
    }
}
