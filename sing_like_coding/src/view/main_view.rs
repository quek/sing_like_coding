use std::{
    collections::HashMap,
    ops::Range,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use common::{
    dsp::{db_from_norm, db_to_norm},
    protocol::MainToPlugin,
};
use eframe::egui::{CentralPanel, Color32, Key, ScrollArea, TopBottomPanel, Ui};

use crate::{
    app_state::{
        AppState, CursorTrack, FocusedPart, LaneCommand, MixerCommand, ModuleCommand, TrackCommand,
        UiCommand,
    },
    device::Device,
    singer::SingerCommand,
    util::with_font_mono,
};

use super::{
    db_slider::DbSlider,
    knob::Knob,
    root_view::Route,
    shortcut_key::{shortcut_key, Modifier},
    stereo_peak_meter::{StereoPeakLevelState, StereoPeakMeter, DB_MAX, DB_MIN},
    util::LabelBuilder,
};

const DEFAULT_TRACK_WIDTH: f32 = 64.0;

pub struct MainView {
    shortcut_map_common: HashMap<(Modifier, Key), UiCommand>,
    shortcut_map_track: HashMap<(Modifier, Key), UiCommand>,
    shortcut_map_lane: HashMap<(Modifier, Key), UiCommand>,
    shortcut_map_module: HashMap<(Modifier, Key), UiCommand>,
    shortcut_map_mixer: HashMap<(Modifier, Key), UiCommand>,
    stereo_peak_level_states: Vec<StereoPeakLevelState>,
    height_line: f32,
    height_mixer: f32,
    height_modules: f32,
    height_track_header: f32,
}

impl MainView {
    pub fn new() -> Self {
        let shortcut_map_common = [
            ((Modifier::None, Key::M), UiCommand::TrackMute(None, None)),
            ((Modifier::None, Key::P), UiCommand::Loop),
            ((Modifier::S, Key::P), UiCommand::Follow),
            ((Modifier::None, Key::S), UiCommand::TrackSolo(None, None)),
            ((Modifier::C, Key::S), UiCommand::SongSave),
            ((Modifier::C, Key::T), UiCommand::TrackAdd),
            ((Modifier::CS, Key::T), UiCommand::LaneAdd),
        ];
        let shortcut_map_track = [
            (
                (Modifier::None, Key::H),
                UiCommand::Track(TrackCommand::CursorLeft),
            ),
            (
                (Modifier::None, Key::L),
                UiCommand::Track(TrackCommand::CursorRight),
            ),
            (
                (Modifier::C, Key::H),
                UiCommand::Track(TrackCommand::MoveLeft),
            ),
            (
                (Modifier::C, Key::L),
                UiCommand::Track(TrackCommand::MoveRight),
            ),
            ((Modifier::C, Key::C), UiCommand::Track(TrackCommand::Copy)),
            ((Modifier::C, Key::X), UiCommand::Track(TrackCommand::Cut)),
            ((Modifier::C, Key::V), UiCommand::Track(TrackCommand::Paste)),
            (
                (Modifier::None, Key::D),
                UiCommand::Track(TrackCommand::Dup),
            ),
            (
                (Modifier::None, Key::Delete),
                UiCommand::Track(TrackCommand::Delete),
            ),
        ];
        let shortcut_map_lane = [
            (
                (Modifier::C, Key::J),
                UiCommand::Lane(LaneCommand::NoteUpdate(-1, 0, 0, false)),
            ),
            (
                (Modifier::C, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::NoteUpdate(-1, 0, 0, false)),
            ),
            (
                (Modifier::C, Key::K),
                UiCommand::Lane(LaneCommand::NoteUpdate(1, 0, 0, false)),
            ),
            (
                (Modifier::C, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::NoteUpdate(1, 0, 0, false)),
            ),
            (
                (Modifier::C, Key::H),
                UiCommand::Lane(LaneCommand::NoteUpdate(-12, 0, 0, false)),
            ),
            (
                (Modifier::C, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::NoteUpdate(-12, 0, 0, false)),
            ),
            (
                (Modifier::C, Key::L),
                UiCommand::Lane(LaneCommand::NoteUpdate(12, 0, 0, false)),
            ),
            (
                (Modifier::C, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::NoteUpdate(12, 0, 0, false)),
            ),
            (
                (Modifier::A, Key::J),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, -1, 0, false)),
            ),
            (
                (Modifier::A, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, -1, 0, false)),
            ),
            (
                (Modifier::A, Key::K),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 1, 0, false)),
            ),
            (
                (Modifier::A, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 1, 0, false)),
            ),
            (
                (Modifier::A, Key::H),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, -0x10, 0, false)),
            ),
            (
                (Modifier::A, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, -0x10, 0, false)),
            ),
            (
                (Modifier::A, Key::L),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0x10, 0, false)),
            ),
            (
                (Modifier::A, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0x10, 0, false)),
            ),
            (
                (Modifier::CA, Key::J),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, -1, false)),
            ),
            (
                (Modifier::CA, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, -1, false)),
            ),
            (
                (Modifier::CA, Key::K),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, 1, false)),
            ),
            (
                (Modifier::CA, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, 1, false)),
            ),
            (
                (Modifier::CA, Key::H),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, -0x10, false)),
            ),
            (
                (Modifier::CA, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, -0x10, false)),
            ),
            (
                (Modifier::CA, Key::L),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, 0x10, false)),
            ),
            (
                (Modifier::CA, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, 0x10, false)),
            ),
            (
                (Modifier::CAS, Key::J),
                UiCommand::Lane(LaneCommand::NoteMove(0, -1)),
            ),
            (
                (Modifier::CAS, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::NoteMove(0, -1)),
            ),
            (
                (Modifier::CAS, Key::K),
                UiCommand::Lane(LaneCommand::NoteMove(0, 1)),
            ),
            (
                (Modifier::CAS, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::NoteMove(0, 1)),
            ),
            (
                (Modifier::CAS, Key::H),
                UiCommand::Lane(LaneCommand::NoteMove(-1, 0)),
            ),
            (
                (Modifier::CAS, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::NoteMove(-1, 0)),
            ),
            (
                (Modifier::CAS, Key::L),
                UiCommand::Lane(LaneCommand::NoteMove(1, 0)),
            ),
            (
                (Modifier::CAS, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::NoteMove(1, 0)),
            ),
            (
                (Modifier::None, Key::J),
                UiCommand::Lane(LaneCommand::CursorDown),
            ),
            (
                (Modifier::None, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::CursorDown),
            ),
            (
                (Modifier::None, Key::K),
                UiCommand::Lane(LaneCommand::CursorUp),
            ),
            (
                (Modifier::None, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::CursorUp),
            ),
            (
                (Modifier::None, Key::H),
                UiCommand::Lane(LaneCommand::CursorLeft),
            ),
            (
                (Modifier::None, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::CursorLeft),
            ),
            (
                (Modifier::None, Key::L),
                UiCommand::Lane(LaneCommand::CursorRight),
            ),
            (
                (Modifier::None, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::CursorRight),
            ),
            ((Modifier::C, Key::C), UiCommand::Lane(LaneCommand::Copy)),
            ((Modifier::C, Key::X), UiCommand::Lane(LaneCommand::Cut)),
            ((Modifier::C, Key::V), UiCommand::Lane(LaneCommand::Paste)),
            ((Modifier::None, Key::D), UiCommand::Lane(LaneCommand::Dup)),
            (
                (Modifier::None, Key::E),
                UiCommand::Lane(LaneCommand::SelectMode),
            ),
            (
                (Modifier::None, Key::Period),
                UiCommand::Lane(LaneCommand::NoteUpdate(0, 0, 0, true)),
            ),
            (
                (Modifier::None, Key::Delete),
                UiCommand::Lane(LaneCommand::NoteDelete),
            ),
            (
                (Modifier::None, Key::Escape),
                UiCommand::Lane(LaneCommand::SelectClear),
            ),
        ];

        let shortcut_map_module = [
            (
                (Modifier::None, Key::K),
                UiCommand::Module(ModuleCommand::CursorUp),
            ),
            (
                (Modifier::None, Key::J),
                UiCommand::Module(ModuleCommand::CursorDown),
            ),
            (
                (Modifier::None, Key::H),
                UiCommand::Module(ModuleCommand::CursorLeft),
            ),
            (
                (Modifier::None, Key::L),
                UiCommand::Module(ModuleCommand::CursorRight),
            ),
        ];
        let shortcut_map_mixer = [
            (
                (Modifier::None, Key::K),
                UiCommand::Mixer(MixerCommand::Volume(1.0)),
            ),
            (
                (Modifier::None, Key::J),
                UiCommand::Mixer(MixerCommand::Volume(-1.0)),
            ),
            (
                (Modifier::S, Key::K),
                UiCommand::Mixer(MixerCommand::Volume(0.1)),
            ),
            (
                (Modifier::S, Key::J),
                UiCommand::Mixer(MixerCommand::Volume(-0.1)),
            ),
            (
                (Modifier::None, Key::H),
                UiCommand::Mixer(MixerCommand::CursorLeft),
            ),
            (
                (Modifier::None, Key::L),
                UiCommand::Mixer(MixerCommand::CursorRight),
            ),
            (
                (Modifier::C, Key::H),
                UiCommand::Mixer(MixerCommand::Pan(-1.0)),
            ),
            (
                (Modifier::C, Key::L),
                UiCommand::Mixer(MixerCommand::Pan(1.0)),
            ),
            (
                (Modifier::S, Key::H),
                UiCommand::Mixer(MixerCommand::Pan(-0.1)),
            ),
            (
                (Modifier::S, Key::L),
                UiCommand::Mixer(MixerCommand::Pan(0.1)),
            ),
        ];

        let shortcut_map_common: HashMap<_, _> = shortcut_map_common.into_iter().collect();
        let shortcut_map_track: HashMap<_, _> = shortcut_map_track.into_iter().collect();
        let shortcut_map_lane: HashMap<_, _> = shortcut_map_lane.into_iter().collect();
        let shortcut_map_module: HashMap<_, _> = shortcut_map_module.into_iter().collect();
        let shortcut_map_mixer: HashMap<_, _> = shortcut_map_mixer.into_iter().collect();

        Self {
            shortcut_map_common,
            shortcut_map_track,
            shortcut_map_lane,
            shortcut_map_module,
            shortcut_map_mixer,
            stereo_peak_level_states: vec![],
            height_line: 30.0,
            height_mixer: 0.0,
            height_modules: 0.0,
            height_track_header: 0.0,
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
                let song_name = format!(
                    "{}{}",
                    if state.song_dirty_p { "*" } else { "" },
                    &state.song.name
                );
                ui.heading(song_name);
                ui.label(format!(
                    "{:.3}ms",
                    state.song_state.process_elasped_avg * 1000.0
                ));
                ui.label(format!("{:.3}%", state.song_state.cpu_usage * 100.0));
            });
        });

        CentralPanel::default().show(gui_context, |ui: &mut Ui| -> anyhow::Result<()> {
            if false {
                ui.label(format!("{:?}", state.cursor_track));
                ui.label(format!("{:?}", state.selection_track_min));
                ui.label(format!("{:?}", state.selection_track_max));
                ui.label(format!("{:?}", state.song_state.tracks[0]));
                ui.label(format!("{:?}", state.song_state.tracks[1]));
                ui.label(format!("{:?}", state.song_state.tracks[2]));
            }
            ui.horizontal(|ui| -> anyhow::Result<()> {
                if ui.button("Play").clicked() {
                    state.sender_to_singer.send(SingerCommand::Play)?;
                }
                if ui.button("Stop").clicked() {
                    state.sender_to_singer.send(SingerCommand::Stop)?;
                }
                ui.label(format!(
                    "{}",
                    play_position_text1(state.song_state.line_play, state.song.lpb)
                ));

                let mut loop_p = state.song_state.loop_p;
                if ui.toggle_value(&mut loop_p, "Loop").clicked() {
                    state.sender_to_singer.send(SingerCommand::Loop)?;
                }

                ui.toggle_value(&mut state.follow_p, "Follow");

                let mut device_start_p = device.as_mut().unwrap().start_p();
                if ui.toggle_value(&mut device_start_p, "Device").clicked() {
                    if device_start_p {
                        device.as_mut().unwrap().stop().unwrap();
                    } else {
                        device.as_mut().unwrap().start().unwrap();
                    }
                }

                Ok(())
            });

            ui.separator();

            let line_range = self.compute_line_range(ui, state);

            with_font_mono(ui, |ui| {
                if state.song_state.play_p && state.follow_p {
                    state.cursor_track.line = state.song_state.line_play
                }

                ui.horizontal(|ui| -> anyhow::Result<()> {
                    self.view_ruler(state, ui, &line_range)?;

                    // 1トラックあたりの幅
                    let track_width = DEFAULT_TRACK_WIDTH as f32;
                    // カーソルの現在のトラック
                    let cursor_track = state.cursor_track.track as f32;
                    // スクロールさせたい位置 = カーソルを中央に持ってくる
                    let visible_track_count = 8.0; // 画面に何トラック出すか（仮）
                    let center_offset = (cursor_track + 0.5) * track_width
                        - (visible_track_count * track_width / 2.0);
                    // clamp: 0 以上に制限（左に行きすぎないように）
                    let scroll_offset = center_offset.max(0.0);

                    ScrollArea::horizontal()
                        .auto_shrink(false)
                        .scroll_offset([scroll_offset, 0.0].into())
                        .show(ui, |ui| -> anyhow::Result<()> {
                            let total_tracks = state.song.tracks.len();

                            let scroll_offset_x = ui.clip_rect().min.x - ui.min_rect().min.x;
                            let visible_width = ui.clip_rect().width();

                            let first_visible_track =
                                (scroll_offset_x / track_width as f32).floor() as usize;
                            let last_visible_track =
                                ((scroll_offset_x + visible_width) / track_width as f32).ceil()
                                    as usize;

                            let first_visible_track = first_visible_track.min(total_tracks);
                            let last_visible_track = last_visible_track.min(total_tracks);

                            for track_index in first_visible_track..last_visible_track {
                                self.view_track(
                                    state,
                                    ui,
                                    track_index,
                                    &line_range,
                                    &mut commands,
                                )?;
                            }

                            Ok(())
                        });
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

    fn compute_line_range(&mut self, ui: &mut Ui, state: &AppState) -> Range<usize> {
        let available_height = ui.available_height()
            - self.height_track_header
            - self.height_modules
            - self.height_mixer;

        let available_rows = (available_height / self.height_line).floor() as usize;

        let center_offset = available_rows / 2;
        let line_start = state.cursor_track.line.saturating_sub(center_offset);
        let line_end = line_start + available_rows;
        let line_range = line_start..line_end;

        // log::debug!(
        //     "{}",
        //     format!(
        //         "{}, {}, {}, {}, {}, {:?}",
        //         available_height,
        //         self.height_track_header,
        //         self.height_line,
        //         self.height_modules,
        //         self.height_mixer,
        //         line_range
        //     )
        // );

        self.height_track_header = 0.0;
        self.height_modules = 0.0;
        self.height_mixer = 0.0;
        self.height_line = 0.0;

        line_range
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
            let map = match state.focused_part {
                crate::app_state::FocusedPart::Track => &self.shortcut_map_track,
                crate::app_state::FocusedPart::Lane => &self.shortcut_map_lane,
                crate::app_state::FocusedPart::Module => &self.shortcut_map_module,
                crate::app_state::FocusedPart::Mixer => &self.shortcut_map_mixer,
            };
            if let Some(command) = map.get(&key) {
                state.run_ui_command(command)?;
            } else if let Some(command) = self.shortcut_map_common.get(&key) {
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

    fn view_mixer(
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
            let mut pan = track.pan;
            let knob = ui.add(Knob { value: &mut pan });
            if knob.dragged() {
                commands.push(UiCommand::TrackPan(track_index, pan));
            } else if knob.double_clicked() {
                commands.push(UiCommand::TrackPan(track_index, 0.5));
            }

            ui.vertical(|ui| -> anyhow::Result<()> {
                let width = 18.0;

                with_font_mono(ui, |ui| {
                    let mut solo = track.solo;
                    if ui
                        .add_sized([width, 0.0], |ui: &mut Ui| ui.toggle_value(&mut solo, "S"))
                        .clicked()
                    {
                        commands.push(UiCommand::TrackSolo(Some(track_index), Some(solo)));
                    }

                    let mut mute = track.mute;
                    if ui
                        .add_sized([width, 0.0], |ui: &mut Ui| ui.toggle_value(&mut mute, "M"))
                        .clicked()
                    {
                        commands.push(UiCommand::TrackMute(Some(track_index), Some(mute)));
                    }
                });

                Ok(())
            });
            Ok(())
        });

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
                bg_color: if state.focused_part == FocusedPart::Mixer
                    && track_index == state.cursor_track.track
                {
                    if SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                        % 1000
                        / 500
                        == 0
                    {
                        Color32::from_rgb(0x55, 0x55, 0)
                    } else {
                        Color32::from_rgb(0x33, 0x33, 0)
                    }
                } else {
                    Color32::BLACK
                },
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

        LabelBuilder::new(
            ui,
            format!("{:.2}dB", db_from_norm(track.volume, DB_MIN, DB_MAX)),
        )
        .build();

        Ok(())
    }

    fn view_lane(
        &mut self,
        state: &AppState,
        ui: &mut Ui,
        track_index: usize,
        lane_index: usize,
        line_range: &Range<usize>,
    ) -> anyhow::Result<()> {
        ui.vertical(|ui| -> anyhow::Result<()> {
            for line in line_range.clone() {
                let mut color = Color32::BLACK;
                if state.cursor_track.track == track_index
                    && state.cursor_track.lane == lane_index
                    && state.cursor_track.line == line
                {
                    color = state.color_cursor(FocusedPart::Lane);
                } else if line == state.song_state.line_play {
                    color = Color32::DARK_GREEN;
                } else if let Some(min) = &state.selection_track_min {
                    if let (min, Some(max)) = if state.select_p {
                        (
                            min.min_merge(&state.cursor_track),
                            Some(min.max_merge(&state.cursor_track)),
                        )
                    } else {
                        (min.clone(), state.selection_track_max.clone())
                    } {
                        let current = CursorTrack {
                            track: track_index,
                            lane: lane_index,
                            line,
                        };
                        if min <= current && current <= max {
                            color = Color32::LIGHT_BLUE;
                        }
                    }
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

                if self.height_line == 0.0 {
                    let height_before = ui.available_height();
                    LabelBuilder::new(ui, text).bg_color(color).build();
                    let height_after = ui.available_height();
                    self.height_line = height_before - height_after;
                } else {
                    LabelBuilder::new(ui, text).bg_color(color).build();
                }
            }
            Ok(())
        });
        Ok(())
    }

    fn view_lanes(
        &mut self,
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
        state: &mut AppState,
        ui: &mut Ui,
        track_index: usize,
        module_index: usize,
    ) -> anyhow::Result<()> {
        let module = &state.song.tracks[track_index].modules[module_index];
        let color = if state.cursor_track.track == track_index
            && state.cursor_module.index == module_index
        {
            state.color_cursor(FocusedPart::Module)
        } else {
            Color32::BLACK
        };
        let label = LabelBuilder::new(ui, &module.name)
            .bg_color(color)
            .size([DEFAULT_TRACK_WIDTH, 0.0])
            .build();
        if label.clicked() {
            state.send_to_plugin(
                MainToPlugin::GuiOpen(track_index, module_index),
                Box::new(|_, _| Ok(())),
            )?;
        }
        label.context_menu(|ui: &mut Ui| {
            if ui.button("Delete").clicked() {
                state
                    .sender_to_singer
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

        let color = if state.cursor_track.track == track_index
            && state.cursor_module.index == state.song.tracks[track_index].modules.len()
        {
            state.color_cursor(FocusedPart::Module)
        } else {
            Color32::BLACK
        };

        if LabelBuilder::new(ui, "+")
            .bg_color(color)
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
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.label(" ");
                for line in line_range.clone() {
                    let color = if line == state.song_state.line_play {
                        Color32::DARK_GREEN
                    } else if (state.song_state.loop_start..state.song_state.loop_start)
                        .contains(&(line * 0x100))
                    {
                        Color32::from_rgb(0x00, 0x30, 0x00)
                    } else {
                        Color32::BLACK
                    };
                    let text = if line % 4 == 0 {
                        play_position_text2(line, state.song.lpb)
                    } else {
                        "".to_string()
                    };
                    LabelBuilder::new(ui, text).bg_color(color).build();
                }
            });

            ui.vertical(|ui| {
                ui.label(" ");
                for line in line_range.clone() {
                    let color = if line == state.song_state.line_play {
                        Color32::DARK_GREEN
                    } else if (state.song_state.loop_start..state.song_state.loop_start)
                        .contains(&(line * 0x100))
                    {
                        Color32::from_rgb(0x00, 0x30, 0x00)
                    } else {
                        Color32::BLACK
                    };
                    LabelBuilder::new(ui, format!("{:02X}", line))
                        .bg_color(color)
                        .build();
                }
            });
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
                let height_before_track_header = ui.available_height();
                LabelBuilder::new(ui, format!("{:<9}", state.song.tracks[track_index].name))
                    .bg_color(if state.cursor_track.track == track_index {
                        state.color_cursor(FocusedPart::Track)
                    } else {
                        Color32::BLACK
                    })
                    .build();
                let height_after_track_header = ui.available_height();
                if self.height_track_header == 0.0 {
                    self.height_track_header =
                        height_before_track_header - height_after_track_header;
                }

                self.view_lanes(state, ui, track_index, line_range).unwrap();
            });

            let inner = ui
                .vertical(|ui| -> anyhow::Result<()> { self.view_modules(state, ui, track_index) });
            if self.height_modules == 0.0 {
                self.height_modules = inner.response.rect.height();
            }

            let inner = ui.vertical(|ui| -> anyhow::Result<()> {
                self.view_mixer(state, ui, track_index, &mut commands)
            });
            if self.height_mixer == 0.0 {
                self.height_mixer = inner.response.rect.height();
            }

            Ok(())
        });
        Ok(())
    }
}

fn play_position_text1(line: usize, lpb: u16) -> String {
    format!(
        "{}.{:X}",
        play_position_text2(line, lpb),
        line % lpb as usize + 1
    )
}

fn play_position_text2(line: usize, lpb: u16) -> String {
    let lpb = lpb as usize;
    let bar = lpb * 4;
    format!("{:03X}.{:X}", line / bar + 1, line % bar / lpb + 1)
}
