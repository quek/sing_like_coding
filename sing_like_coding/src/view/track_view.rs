use std::{collections::HashMap, ops::Range};

use anyhow::Result;
use common::{
    dsp::{db_from_norm, db_to_norm},
    protocol::MainToPlugin,
};
use eframe::egui::{CentralPanel, Color32, Frame, Key, TopBottomPanel, Ui};

use crate::{
    app_state::{AppState, UiCommand},
    device::Device,
    singer::SingerCommand,
    util::with_font_mono,
};

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

        let mut commands = vec![];

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

            ui.separator();

            with_font_mono(ui, |ui| {
                let line_start = (state.cursor.line as i64 - 0x0f).max(0) as usize;
                let line_end = line_start + 0x20;
                let line_range = line_start..line_end;
                ui.horizontal(|ui| -> anyhow::Result<()> {
                    self.view_ruler(state, ui, &line_range)?;

                    for track_index in 0..state.song.tracks.len() {
                        self.view_track(state, ui, track_index, &line_range, &mut commands)?;
                    }
                    Ok(())
                });
            });
            Ok(())
        });

        for command in commands {
            state.run_ui_command(&command)?;
        }

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

    fn view_fader(
        &mut self,
        state: &AppState,
        ui: &mut Ui,
        track_index: usize,
        commands: &mut Vec<UiCommand>,
    ) -> anyhow::Result<()> {
        let track = &state.song.tracks[track_index];
        let peak_level_state = self.stereo_peak_level_state(track_index);
        peak_level_state.update(&state.song_state.tracks[track_index].peaks);
        for x in [&peak_level_state.left, &peak_level_state.right] {
            LabelBuilder::new(ui, format!("{:.2}dB", x.hold_db)).build();
        }

        ui.horizontal(|ui| -> anyhow::Result<()> {
            let height = 160.0;

            ui.add(StereoPeakMeter {
                peak_level_state,
                min_db: DB_MIN,
                max_db: DB_MAX,
                show_scale: true,
                height,
            });

            let mut db_value = db_from_norm(track.volume as f32, DB_MIN, DB_MAX);
            let fader = ui.add(DbSlider {
                db_value: &mut db_value,
                min_db: DB_MIN,
                max_db: DB_MAX,
                height,
            });
            if fader.dragged() {
                commands.push(UiCommand::TrackVolume(
                    track_index,
                    db_to_norm(db_value, DB_MIN, DB_MAX),
                ));
            } else if fader.double_clicked() {
                commands.push(UiCommand::TrackVolume(
                    track_index,
                    db_to_norm(0.0, DB_MIN, DB_MAX),
                ));
            }

            Ok(())
        });
        Ok(())
    }

    fn view_lane(
        &self,
        state: &AppState,
        ui: &mut Ui,
        track_index: usize,
        lane_index: usize,
        line_range: &Range<usize>,
    ) -> anyhow::Result<()> {
        ui.vertical(|ui| -> anyhow::Result<()> {
            for line in line_range.clone() {
                let color = if state.cursor.track == track_index
                    && state.cursor.lane == lane_index
                    && state.cursor.line == line
                {
                    Color32::YELLOW
                } else if line == state.song_state.line_play {
                    Color32::DARK_GREEN
                } else if state.selected_cells.contains(&(track_index, line)) {
                    Color32::LIGHT_BLUE
                } else {
                    Color32::BLACK
                };
                let text = state.song.tracks[track_index].lanes[lane_index]
                    .note(line)
                    .map_or("--- -- --".to_string(), |note| {
                        if note.off {
                            format!("{:<3}    {:02X}", note.note_name(), note.delay)
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
        Ok(())
    }

    fn view_lanes(
        &self,
        state: &AppState,
        ui: &mut Ui,
        track_index: usize,
        line_range: &Range<usize>,
    ) -> anyhow::Result<()> {
        ui.horizontal(|ui| -> anyhow::Result<()> {
            for lane_index in 0..state.song.tracks[track_index].lanes.len() {
                self.view_lane(state, ui, track_index, lane_index, line_range)?;
            }
            Ok(())
        });
        Ok(())
    }

    fn view_module(
        &self,
        state: &AppState,
        ui: &mut Ui,
        track_index: usize,
        module_index: usize,
    ) -> anyhow::Result<()> {
        let module = &state.song.tracks[track_index].modules[module_index];
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
                    .send(SingerCommand::PluginDelete(track_index, module_index))
                    .unwrap();
                ui.close_menu();
            }
        });
        Ok(())
    }

    fn view_modules(
        &self,
        state: &mut AppState,
        ui: &mut Ui,
        track_index: usize,
    ) -> anyhow::Result<()> {
        for module_index in 0..state.song.tracks[track_index].modules.len() {
            self.view_module(state, ui, track_index, module_index)?;
        }
        if LabelBuilder::new(ui, "+")
            .size([DEFAULT_TRACK_WIDTH, 0.0])
            .build()
            .clicked()
        {
            state.route = Route::PluginSelect;
        }

        Ok(())
    }

    fn view_ruler(
        &self,
        state: &AppState,
        ui: &mut Ui,
        line_range: &Range<usize>,
    ) -> anyhow::Result<()> {
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
        Ok(())
    }

    fn view_track(
        &mut self,
        state: &mut AppState,
        ui: &mut Ui,
        track_index: usize,
        line_range: &Range<usize>,
        mut commands: &mut Vec<UiCommand>,
    ) -> anyhow::Result<()> {
        ui.vertical(|ui| -> anyhow::Result<()> {
            with_font_mono(ui, |ui| {
                LabelBuilder::new(ui, format!("{:<9}", state.song.tracks[track_index].name))
                    .bg_color(if state.selected_tracks.contains(&track_index) {
                        Color32::GREEN
                    } else {
                        Color32::BLACK
                    })
                    .build();

                self.view_lanes(state, ui, track_index, line_range).unwrap();
            });

            self.view_modules(state, ui, track_index)?;

            self.view_fader(state, ui, track_index, &mut commands)?;

            Ok(())
        });
        Ok(())
    }
}
