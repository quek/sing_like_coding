use std::{
    collections::HashMap,
    ops::Range,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use clap_sys::id::clap_id;
use common::{
    dsp::db_to_norm, event::Event, module::Module, process_track_context::ProcessTrackContext,
};
use serde::{Deserialize, Serialize};

use crate::view::stereo_peak_meter::{DB_MAX, DB_MIN};

use super::{lane::Lane, lane_item::LaneItem, note::Note};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub volume: f32,
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub modules: Vec<Module>,
    pub lanes: Vec<Lane>,
    pub automation_params: Vec<(usize, clap_id)>, // (module_index, param_id)
    #[serde(skip_serializing, skip_deserializing)]
    on_key_lane_map: HashMap<i16, usize>,
}

impl Track {
    pub fn new() -> Self {
        Self {
            name: "T01".to_string(),
            volume: db_to_norm(0.0, DB_MIN, DB_MAX),
            pan: 0.5,
            solo: false,
            mute: false,
            modules: vec![],
            lanes: vec![Lane::new()],
            automation_params: vec![],
            on_key_lane_map: Default::default(),
        }
    }

    pub fn process_module(
        &self,
        track_index: usize,
        context: &mut ProcessTrackContext,
        module_index: usize,
        contexts: &Vec<Arc<Mutex<ProcessTrackContext>>>,
    ) -> Result<()> {
        self.prepare_module_event(context, module_index)?;
        self.prepare_module_audio(track_index, context, module_index, contexts)?;
        context.plugins[module_index].process()?;
        Ok(())
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

    pub fn events_append(
        &mut self,
        events: &Vec<Event>,
        play_position: &Range<usize>,
    ) -> Result<()> {
        let line = play_position.start / 0x100;
        let delay = (play_position.start % 0x100) as u8;
        for event in events {
            match event {
                Event::NoteOn(key, velocity, _) => {
                    let lane_item = LaneItem::Note(Note {
                        key: *key,
                        velocity: *velocity,
                        delay,
                        ..Default::default()
                    });
                    for lane_index in 0..usize::MAX {
                        if self.lanes.len() - 1 < lane_index {
                            self.lane_add();
                        }
                        if !self.lanes[lane_index].items.contains_key(&line) {
                            self.lanes[lane_index].items.insert(line, lane_item);
                            self.on_key_lane_map.insert(*key, lane_index);
                            break;
                        }
                    }
                }
                Event::NoteOff(key, _) => {
                    // TODO 1 line で On, Off あると Off だけになる
                    // Cut FX コマンドの実装が必要
                    if let Some(lane_index) = self.on_key_lane_map.get(key) {
                        let lane_item = LaneItem::Note(Note {
                            key: *key,
                            off: true,
                            delay,
                            ..Default::default()
                        });
                        self.lanes[*lane_index].items.insert(line, lane_item);
                    }
                }
                Event::NoteAllOff => continue,
                Event::ParamValue(_, _, _, _) => continue,
            }
        }
        Ok(())
    }

    pub fn lane_add(&mut self) {
        self.lanes.push(Lane::new());
    }

    fn prepare_module_audio(
        &self,
        track_index: usize,
        context: &mut ProcessTrackContext,
        module_index: usize,
        contexts: &Vec<Arc<Mutex<ProcessTrackContext>>>,
    ) -> Result<()> {
        for autdio_input in self.modules[module_index].audio_inputs.iter() {
            let src_ptr = if autdio_input.src_module_index.0 == track_index {
                context.plugins[autdio_input.src_module_index.1].ptr
            } else {
                let context = contexts[autdio_input.src_module_index.0].lock().unwrap();
                context.plugins[autdio_input.src_module_index.1].ptr
            };
            let src_process_data = unsafe { &*src_ptr };
            let src_constant_mask = src_process_data.constant_mask_out[autdio_input.src_port_index];
            let src_buffer = &src_process_data.buffer_out[autdio_input.src_port_index];
            let src_nchannels = src_process_data.nchannels_out[autdio_input.src_port_index];
            let dst_process_data = context.plugins[module_index].process_data_mut();
            let dst_constant_mask =
                &mut dst_process_data.constant_mask_in[autdio_input.dst_port_index];
            let dst_buffer = &mut dst_process_data.buffer_in[autdio_input.dst_port_index];
            let dst_nchannels = dst_process_data.nchannels_in[autdio_input.dst_port_index];

            match (src_nchannels, dst_nchannels) {
                (src_nchannels, dst_nchannels) if src_nchannels == dst_nchannels => {
                    for ch in 0..src_nchannels {
                        let constant_mask_bit = 1 << ch;
                        if (src_constant_mask & constant_mask_bit) == 0 {
                            *dst_constant_mask &= !constant_mask_bit;
                            dst_buffer[ch].copy_from_slice(&src_buffer[ch]);
                        } else {
                            *dst_constant_mask |= constant_mask_bit;
                            dst_buffer[ch][0] = src_buffer[ch][0];
                        }
                    }
                }
                (1, dst_nchannels) => {
                    for ch in 0..dst_nchannels {
                        let constant_mask_bit = 1 << ch;
                        if (src_constant_mask & 1) == 0 {
                            *dst_constant_mask &= !constant_mask_bit;
                            dst_buffer[ch].copy_from_slice(&src_buffer[0]);
                        } else {
                            *dst_constant_mask |= constant_mask_bit;
                            dst_buffer[ch][0] = src_buffer[0][0];
                        }
                    }
                }
                (src_nchannels, 1) => {
                    for ch in 0..src_nchannels {
                        let constant_mask_bit = 1 << ch;
                        if (src_constant_mask & constant_mask_bit) == 0 {
                            for i in 0..context.nframes {
                                if ch == 0 {
                                    dst_buffer[0][i] = src_buffer[ch][i] / src_nchannels as f32;
                                } else {
                                    dst_buffer[0][i] += src_buffer[ch][i] / src_nchannels as f32;
                                }
                            }
                        } else {
                            for i in 0..context.nframes {
                                if ch == 0 {
                                    dst_buffer[0][i] = src_buffer[ch][0] / src_nchannels as f32;
                                } else {
                                    dst_buffer[0][i] += src_buffer[ch][0] / src_nchannels as f32;
                                }
                            }
                        }
                    }
                    *dst_constant_mask = 0;
                }
                (src_nchannels, dst_nchannels) => {
                    for ch in 0..(src_nchannels.min(dst_nchannels)) {
                        let constant_mask_bit = 1 << ch;
                        if (src_constant_mask & constant_mask_bit) == 0 {
                            *dst_constant_mask &= !constant_mask_bit;
                            dst_buffer[ch].copy_from_slice(&src_buffer[ch]);
                        } else {
                            *dst_constant_mask |= constant_mask_bit;
                            dst_buffer[ch][0] = src_buffer[ch][0];
                        }
                    }
                }
            }
        }

        Ok(())
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
