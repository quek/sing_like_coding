use std::collections::HashMap;

use anyhow::Result;
use common::{dsp::db_from_norm, protocol::MainToPlugin};
use eframe::egui::{CentralPanel, Color32, Frame, Key, Label, TopBottomPanel, Ui};

use crate::{app_state::AppState, device::Device, singer::SingerCommand, util::with_font_mono};

use super::{
    db_slider::DbSlider,
    main_view::Route,
    shortcut_key::{shortcut_key, Modifier},
    stereo_peak_meter::{StereoPeakLevelState, StereoPeakMeter, DB_MAX, DB_MIN},
    util::LabelBuilder,
};

const DEFAULT_TRACK_WIDTH: f32 = 64.0;

pub struct TrackView {
    shortcut_map: HashMap<(Modifier, Key), UiCommand>,
    stereo_peak_level_states: Vec<StereoPeakLevelState>,
}

impl TrackView {
    pub fn new() -> Self {
        let shortcut_map = [
            (
                (Modifier::C, Key::J),
                UiCommand::NoteUpdate(-1, 0, 0, false),
            ),
            ((Modifier::C, Key::K), UiCommand::NoteUpdate(1, 0, 0, false)),
            (
                (Modifier::C, Key::H),
                UiCommand::NoteUpdate(-12, 0, 0, false),
            ),
            (
                (Modifier::C, Key::L),
                UiCommand::NoteUpdate(12, 0, 0, false),
            ),
            ((Modifier::C, Key::T), UiCommand::TrackAdd),
            ((Modifier::CS, Key::T), UiCommand::LaneAdd),
            (
                (Modifier::A, Key::J),
                UiCommand::NoteUpdate(0, -1, 0, false),
            ),
            ((Modifier::A, Key::K), UiCommand::NoteUpdate(0, 1, 0, false)),
            (
                (Modifier::A, Key::H),
                UiCommand::NoteUpdate(0, -0x10, 0, false),
            ),
            (
                (Modifier::A, Key::L),
                UiCommand::NoteUpdate(0, 0x10, 0, false),
            ),
            (
                (Modifier::CA, Key::J),
                UiCommand::NoteUpdate(0, 0, -1, false),
            ),
            (
                (Modifier::CA, Key::K),
                UiCommand::NoteUpdate(0, 0, 1, false),
            ),
            (
                (Modifier::CA, Key::H),
                UiCommand::NoteUpdate(0, 0, -0x10, false),
            ),
            (
                (Modifier::CA, Key::L),
                UiCommand::NoteUpdate(0, 0, 0x10, false),
            ),
            ((Modifier::None, Key::J), UiCommand::CursorDown),
            ((Modifier::None, Key::K), UiCommand::CursorUp),
            ((Modifier::None, Key::H), UiCommand::CursorLeft),
            ((Modifier::None, Key::L), UiCommand::CursorRight),
            (
                (Modifier::None, Key::Period),
                UiCommand::NoteUpdate(0, 0, 0, true),
            ),
            ((Modifier::None, Key::Delete), UiCommand::NoteDelte),
        ];
        let shortcut_map: HashMap<_, _> = shortcut_map.into_iter().collect();

        Self {
            shortcut_map,
            stereo_peak_level_states: vec![],
        }
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
                                } else if (state.song_state.loop_start..state.song_state.loop_start)
                                    .contains(&(line * 0x100))
                                {
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
                                                        if note.off {
                                                            format!(
                                                                "{:<3}    {:02X}",
                                                                note.note_name(),
                                                                note.delay
                                                            )
                                                        } else {
                                                            format!(
                                                                "{:<3} {:02X} {:02X}",
                                                                note.note_name(),
                                                                note.velocity as i32,
                                                                note.delay
                                                            )
                                                        }
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

                            let peak_level_state = self.stereo_peak_level_state(track_index);
                            peak_level_state.update(&state.song_state.tracks[track_index].peaks);
                            for x in [&peak_level_state.left, &peak_level_state.right] {
                                LabelBuilder::new(ui, format!("{:.2}dB", x.hold_db)).build();
                            }

                            ui.horizontal(|ui| {
                                let height = 160.0;

                                ui.add(StereoPeakMeter {
                                    peak_level_state,
                                    min_db: DB_MIN,
                                    max_db: DB_MAX,
                                    show_scale: true,
                                    height,
                                });

                                let mut db_value =
                                    db_from_norm(track.volume as f32, DB_MIN, DB_MAX);
                                ui.add(DbSlider {
                                    db_value: &mut db_value,
                                    min_db: DB_MIN,
                                    max_db: DB_MAX,
                                    height,
                                });
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
        let focused = gui_context.memory(|memory| memory.focused());
        if focused.is_some() {
            return Ok(());
        }

        if let Some(key) = shortcut_key(gui_context) {
            if let Some(command) = self.shortcut_map.get(&key) {
                state.run_ui_command(command)?;
            }
        }

        Ok(())
    }

    fn stereo_peak_level_state(&mut self, track_index: usize) -> &mut StereoPeakLevelState {
        self.stereo_peak_level_states
            .resize_with(track_index + 1, Default::default);
        &mut self.stereo_peak_level_states[track_index]
    }
}

// #[derive(Default)]
// pub struct NoteUpdate {
//     pub key_delta: i16,
//     pub velociy_delta: i16,
//     pub delay_delta: i16,
//     pub off: bool,
// }

pub enum UiCommand {
    NoteUpdate(i16, i16, i16, bool),
    NoteDelte,
    TrackAdd,
    LaneAdd,
    CursorUp,
    CursorDown,
    CursorLeft,
    CursorRight,
}
