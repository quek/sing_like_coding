use anyhow::Result;
use common::protocol::MainToPlugin;
use eframe::egui::{CentralPanel, Color32, Frame, Key, Label, TopBottomPanel, Ui};

use crate::{
    app_state::AppState,
    command::{track_add::TrackAdd, Command},
    device::Device,
    singer::SingerCommand,
    util::with_font_mono,
};

use super::{
    main_view::Route,
    shortcut_key::{Modifier, ShortcutKey},
    util::LabelBuilder,
};

const DEFAULT_TRACK_WIDTH: f32 = 64.0;

pub struct TrackView {}

impl TrackView {
    pub fn new() -> Self {
        Self {}
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut AppState,
        device: &mut Option<Device>,
    ) -> Result<()> {
        self.process_shortcut(gui_context, state)?;

        TopBottomPanel::top("Top").show(gui_context, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Sing Like Coding");
                ui.label(format!(
                    "{:.3}ms",
                    state.song_state.process_elasped_avg * 1000.0
                ));
                ui.label(format!("{:.3}%", state.song_state.cpu_usage * 100.0));
            });
        });

        CentralPanel::default().show(gui_context, |ui: &mut Ui| -> anyhow::Result<()> {
            ui.horizontal(|ui| {
                if ui.button("device start").clicked() {
                    device.as_mut().unwrap().start().unwrap();
                }
                if ui.button("device stop").clicked() {
                    device.as_mut().unwrap().stop().unwrap();
                }
            });

            ui.separator();

            ui.horizontal(|ui| -> anyhow::Result<()> {
                if ui.button("Play").clicked() {
                    state.view_sender.send(SingerCommand::Play)?;
                }
                if ui.button("Stop").clicked() {
                    state.view_sender.send(SingerCommand::Stop)?;
                }
                ui.label(format!("Line {:04}", state.song_state.line_play));
                let mut loop_p = state.song_state.loop_p;
                if ui.toggle_value(&mut loop_p, "Loop").clicked() {
                    state.view_sender.send(SingerCommand::Loop)?;
                }
                Ok(())
            });

            // ui.separator();

            // ui.horizontal(|ui| {
            //     if ui.button("Note On").clicked() {
            //         let track_index = 0;
            //         let key = 63;
            //         let channel = 0;
            //         let velocity = 100.0;
            //         let time = 0;
            //         state
            //             .view_sender
            //             .send(SingerMsg::NoteOn(track_index, key, channel, velocity, time))
            //             .unwrap();
            //     }

            //     if ui.button("Note Off").clicked() {
            //         let track_index = 0;
            //         let key = 63;
            //         let channel = 0;
            //         let velocity = 0.0;
            //         let time = 0;
            //         state
            //             .view_sender
            //             .send(SingerMsg::NoteOff(
            //                 track_index,
            //                 key,
            //                 channel,
            //                 velocity,
            //                 time,
            //             ))
            //             .unwrap();
            //     }
            // });

            ui.separator();

            with_font_mono(ui, |ui| {
                let line_start = (state.cursor.line as i64 - 0x0f).max(0) as usize;
                let line_end = line_start + 0x20;
                let line_range = line_start..line_end;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(" ");
                        for line in line_range.clone() {
                            Frame::NONE
                                .fill(if line == state.song_state.line_play {
                                    Color32::DARK_GREEN
                                } else if state.song_state.loop_range.contains(&(line * 0x100)) {
                                    Color32::from_rgb(0x00, 0x30, 0x00)
                                } else {
                                    Color32::BLACK
                                })
                                .show(ui, |ui| {
                                    ui.label(format!("{:02X}", line));
                                });
                        }
                    });
                    for (track_index, track) in state.song.tracks.iter_mut().enumerate() {
                        ui.vertical(|ui| -> anyhow::Result<()> {
                            with_font_mono(ui, |ui| {
                                Frame::NONE
                                    .fill(if state.selected_tracks.contains(&track_index) {
                                        Color32::GREEN
                                    } else {
                                        Color32::BLACK
                                    })
                                    .show(ui, |ui| {
                                        ui.add(Label::new(format!("{:<9}", track.name)).truncate());
                                    });
                                ui.horizontal(|ui| -> anyhow::Result<()> {
                                    for lane_index in 0..track.lanes.len() {
                                        ui.vertical(|ui| -> anyhow::Result<()> {
                                            for line in line_range.clone() {
                                                let color = if state.cursor.track == track_index
                                                    && state.cursor.lane == lane_index
                                                    && state.cursor.line == line
                                                {
                                                    Color32::YELLOW
                                                } else if line == state.song_state.line_play {
                                                    Color32::DARK_GREEN
                                                } else if state
                                                    .selected_cells
                                                    .contains(&(track_index, line))
                                                {
                                                    Color32::LIGHT_BLUE
                                                } else {
                                                    Color32::BLACK
                                                };
                                                let text = track.lanes[lane_index]
                                                    .note(line)
                                                    .map_or("--- -- --".to_string(), |note| {
                                                        format!(
                                                            "{:<3} {:02X} {:02X}",
                                                            note.note_name(),
                                                            note.velocity as i32,
                                                            note.delay
                                                        )
                                                    });

                                                LabelBuilder::new(ui, text).bg_color(color).build();
                                            }
                                            Ok(())
                                        });
                                    }
                                    Ok(())
                                });
                            });

                            for (module_index, module) in track.modules.iter_mut().enumerate() {
                                let label = LabelBuilder::new(ui, &module.name)
                                    .size([DEFAULT_TRACK_WIDTH, 0.0])
                                    .build();
                                if label.clicked() {
                                    state
                                        .sender_to_loop
                                        .send(MainToPlugin::GuiOpen(track_index, module_index))?;
                                }
                                label.context_menu(|ui: &mut Ui| {
                                    if ui.button("Delete").clicked() {
                                        state
                                            .view_sender
                                            .send(SingerCommand::PluginDelete(
                                                track_index,
                                                module_index,
                                            ))
                                            .unwrap();
                                        ui.close_menu();
                                    }
                                });
                            }

                            if LabelBuilder::new(ui, "+")
                                .size([DEFAULT_TRACK_WIDTH, 0.0])
                                .build()
                                .clicked()
                            {
                                state.route = Route::PluginSelect;
                            }

                            Ok(())
                        });
                    }
                });
            });
            Ok(())
        });

        Ok(())
    }

    fn process_shortcut(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut AppState,
    ) -> Result<()> {
        let input = gui_context.input(|i| i.clone());
        let focused = gui_context.memory(|memory| memory.focused());
        if focused.is_some() {
            return Ok(());
        }

        if input.is(Modifier::C, Key::J) {
            note_update(-1, 0, 0, state);
        } else if input.is(Modifier::C, Key::K) {
            note_update(1, 0, 0, state);
        } else if input.is(Modifier::C, Key::H) {
            note_update(-12, 0, 0, state);
        } else if input.is(Modifier::C, Key::L) {
            note_update(12, 0, 0, state);
        } else if input.is(Modifier::C, Key::T) {
            TrackAdd {}.call(state)?;
        } else if input.is(Modifier::CS, Key::T) {
            state
                .view_sender
                .send(SingerCommand::LaneAdd(state.cursor.track))?;
        } else if input.is(Modifier::A, Key::J) {
            note_update(0, -1, 0, state);
        } else if input.is(Modifier::A, Key::K) {
            note_update(0, 1, 0, state);
        } else if input.is(Modifier::A, Key::H) {
            note_update(0, -0x10, 0, state);
        } else if input.is(Modifier::A, Key::L) {
            note_update(0, 0x10, 0, state);
        } else if input.is(Modifier::CA, Key::J) {
            note_update(0, 0, -1, state);
        } else if input.is(Modifier::CA, Key::K) {
            note_update(0, 0, 1, state);
        } else if input.is(Modifier::CA, Key::H) {
            note_update(0, 0, -0x10, state);
        } else if input.is(Modifier::CA, Key::L) {
            note_update(0, 0, 0x10, state);
        } else if input.is(Modifier::None, Key::J) {
            state.cursor_down();
        } else if input.is(Modifier::None, Key::K) {
            state.cursor_up();
        } else if input.is(Modifier::None, Key::H) {
            state.cursor_left();
        } else if input.is(Modifier::None, Key::L) {
            state.cursor_right();
        } else if input.is(Modifier::None, Key::Delete) {
            state
                .view_sender
                .send(SingerCommand::NoteDelete(state.cursor.clone()))
                .unwrap();
        }

        Ok(())
    }
}

fn note_update(key_delta: i16, velociy_delta: i16, delay_delta: i16, state: &mut AppState) {
    if let Some(note) = state.song.note(&state.cursor) {
        let mut note = note.clone();
        note.key = (note.key + key_delta).clamp(0, 127);
        note.velocity = (note.velocity + velociy_delta as f64).clamp(0.0, 127.0);
        note.delay = (note.delay as i16 + delay_delta).clamp(0, 0xff) as u8;
        state.note_last = note;
    }

    let mut note = state.note_last.clone();
    note.line = state.cursor.line;
    state
        .view_sender
        .send(SingerCommand::Note(state.cursor.clone(), note))
        .unwrap();
}
