use anyhow::Result;
use common::protocol::MainToPlugin;
use eframe::egui::{CentralPanel, Color32, Frame, Key, Label, TopBottomPanel, Ui};

use crate::{app_state::AppState, device::Device, singer::SingerMsg, util::with_font_mono};

use super::main_view::Route;

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
                    "{:.6}ms",
                    state.song_state.process_elasped_avg * 1000.0
                ));
                ui.label(format!("{:.6}%", state.song_state.cpu_usage * 100.0));
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
                    state.view_sender.send(SingerMsg::Play)?;
                }
                if ui.button("Stop").clicked() {
                    state.view_sender.send(SingerMsg::Stop)?;
                }
                ui.label(format!("Line {:04}", state.song_state.line_play));
                let mut loop_p = state.song_state.loop_p;
                if ui.toggle_value(&mut loop_p, "Loop").clicked() {
                    state.view_sender.send(SingerMsg::Loop)?;
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
                // TODO
                let line_range = 0..0x30;
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(" ");
                        for line in line_range.clone() {
                            Frame::NONE
                                .fill(if line == state.song_state.line_play {
                                    Color32::DARK_GREEN
                                } else if state.song_state.loop_range.contains(&line) {
                                    Color32::DARK_GRAY
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
                                        ui.label(&track.name);
                                    });
                                for line in line_range.clone() {
                                    let color = if state.cursor_track == track_index
                                        && state.cursor_line == line
                                    {
                                        Color32::YELLOW
                                    } else if line == state.song_state.line_play {
                                        Color32::DARK_GREEN
                                    } else if state.selected_cells.contains(&(track_index, line)) {
                                        Color32::LIGHT_BLUE
                                    } else {
                                        Color32::BLACK
                                    };
                                    Frame::NONE.fill(color).show(ui, |ui| {
                                        let text = track
                                            .note(line)
                                            .map_or("---".to_string(), |note| note.note_name());
                                        ui.label(text);
                                    });
                                }
                            });

                            Frame::NONE
                                .fill(Color32::BLACK)
                                .show(ui, |ui| -> anyhow::Result<()> {
                                    for (module_index, module) in
                                        track.modules.iter_mut().enumerate()
                                    {
                                        if ui
                                            .add_sized(
                                                [DEFAULT_TRACK_WIDTH, 0.0],
                                                Label::new(&module.name).truncate(),
                                            )
                                            .clicked()
                                        {
                                            state.sender_to_loop.send(MainToPlugin::GuiOpen(
                                                track_index,
                                                module_index,
                                            ))?;
                                        }
                                    }

                                    if ui
                                        .add_sized([DEFAULT_TRACK_WIDTH, 0.0], Label::new("+"))
                                        .clicked()
                                    {
                                        state.route = Route::PluginSelect;
                                    }

                                    Ok(())
                                });

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

        if input.modifiers.ctrl && !input.modifiers.alt && !input.modifiers.shift {
            if input.key_pressed(Key::J) {
                note_update(-1, state);
            } else if input.key_pressed(Key::K) {
                note_update(1, state);
            } else if input.key_pressed(Key::H) {
                note_update(-12, state);
            } else if input.key_pressed(Key::L) {
                note_update(12, state);
            }
        } else if !input.modifiers.ctrl && !input.modifiers.alt && !input.modifiers.shift {
            if input.key_pressed(Key::J) {
                state.cursor_line += 1;
            } else if input.key_pressed(Key::K) {
                if state.cursor_line != 0 {
                    state.cursor_line -= 1;
                }
            } else if input.key_pressed(Key::H) {
                if state.cursor_track == 0 {
                    state.cursor_track = state.song.tracks.len() - 1;
                } else {
                    state.cursor_track -= 1;
                }
                state.selected_tracks.clear();
                state.selected_tracks.push(state.cursor_track);
            } else if input.key_pressed(Key::L) {
                if state.cursor_track + 1 == state.song.tracks.len() {
                    state.cursor_track = 0;
                } else {
                    state.cursor_track += 1;
                }
                state.selected_tracks.clear();
                state.selected_tracks.push(state.cursor_track);
            }
        }

        Ok(())
    }
}

fn note_update(key_delta: i16, state: &mut AppState) {
    let key = if let Some(note) = state.song.tracks[state.cursor_track].note(state.cursor_line) {
        note.key + key_delta
    } else {
        state.key_last
    }
    .clamp(0, 127);

    state
        .view_sender
        .send(SingerMsg::Note(state.cursor_track, state.cursor_line, key))
        .unwrap();
    state.key_last = key;
}
