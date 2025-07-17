use std::{collections::HashMap, ops::Range, path::PathBuf};

use anyhow::Result;
use common::{
    dsp::{db_from_norm, db_to_norm},
    protocol::MainToPlugin,
};
use eframe::egui::{
    self, text::LayoutJob, CentralPanel, Color32, DragValue, DroppedFile, FontId, Key, Label,
    TextEdit, TextFormat, TopBottomPanel, Ui,
};

use crate::{
    app_state::{
        AppState, CursorTrack, FocusedPart, LaneCommand, MixerCommand, ModuleCommand, TrackCommand,
        UiCommand,
    },
    device::Device,
    model::lane_item::LaneItem,
    util::with_font_mono,
};

use super::{
    db_slider::DbSlider,
    knob::Knob,
    root_view::Route,
    shortcut_key::{shortcut_key, Modifier},
    stereo_peak_meter::{StereoPeakLevelState, StereoPeakMeter, DB_MAX, DB_MIN},
    util::{select_all_text, LabelBuilder},
};

const DEFAULT_TRACK_WIDTH: f32 = 64.0;

pub struct MainView {
    bpm: Option<f64>,
    dropped_files: Vec<DroppedFile>,
    dropped_files_pre: Vec<DroppedFile>,
    line_play: usize,
    shortcut_map_common: HashMap<(Modifier, Key), UiCommand>,
    shortcut_map_track: HashMap<(Modifier, Key), UiCommand>,
    shortcut_map_lane: HashMap<(Modifier, Key), UiCommand>,
    shortcut_map_pattern: HashMap<(Modifier, Key), UiCommand>,
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
            ((Modifier::None, Key::F), UiCommand::Follow),
            ((Modifier::None, Key::P), UiCommand::Loop),
            ((Modifier::C, Key::P), UiCommand::LoopRange),
            ((Modifier::None, Key::S), UiCommand::TrackSolo(None, None)),
            ((Modifier::C, Key::S), UiCommand::SongSave),
            ((Modifier::C, Key::T), UiCommand::TrackAdd),
            ((Modifier::CS, Key::T), UiCommand::LaneAdd),
            ((Modifier::C, Key::Z), UiCommand::Undo),
            ((Modifier::CS, Key::Z), UiCommand::Redo),
            ((Modifier::None, Key::Period), UiCommand::Repeat),
            ((Modifier::None, Key::Comma), UiCommand::PatternToggle),
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
            (
                (Modifier::None, Key::R),
                UiCommand::Track(TrackCommand::Rename),
            ),
        ];
        let shortcut_map_lane = [
            ((Modifier::None, Key::U), UiCommand::Digit4Times),
            ((Modifier::None, Key::Num0), UiCommand::Digit(0)),
            ((Modifier::None, Key::Num1), UiCommand::Digit(1)),
            ((Modifier::None, Key::Num2), UiCommand::Digit(2)),
            ((Modifier::None, Key::Num3), UiCommand::Digit(3)),
            ((Modifier::None, Key::Num4), UiCommand::Digit(4)),
            ((Modifier::None, Key::Num5), UiCommand::Digit(5)),
            ((Modifier::None, Key::Num6), UiCommand::Digit(6)),
            ((Modifier::None, Key::Num7), UiCommand::Digit(7)),
            ((Modifier::None, Key::Num8), UiCommand::Digit(8)),
            ((Modifier::None, Key::Num9), UiCommand::Digit(9)),
            ((Modifier::None, Key::G), UiCommand::Lane(LaneCommand::Go)),
            (
                (Modifier::C, Key::J),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(-1, 0, 0, None, -1)),
            ),
            (
                (Modifier::C, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(-1, 0, 0, None, -1)),
            ),
            (
                (Modifier::C, Key::K),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(1, 0, 0, None, 1)),
            ),
            (
                (Modifier::C, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(1, 0, 0, None, 1)),
            ),
            (
                (Modifier::C, Key::H),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(-12, 0, 0, None, -0x10)),
            ),
            (
                (Modifier::C, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(-12, 0, 0, None, -0x10)),
            ),
            (
                (Modifier::C, Key::L),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(12, 0, 0, None, 0x10)),
            ),
            (
                (Modifier::C, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(12, 0, 0, None, 0x10)),
            ),
            (
                (Modifier::A, Key::J),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, -1, 0, None, 0)),
            ),
            (
                (Modifier::A, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, -1, 0, None, 0)),
            ),
            (
                (Modifier::A, Key::K),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 1, 0, None, 0)),
            ),
            (
                (Modifier::A, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 1, 0, None, 0)),
            ),
            (
                (Modifier::A, Key::H),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, -0x10, 0, None, 0)),
            ),
            (
                (Modifier::A, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, -0x10, 0, None, 0)),
            ),
            (
                (Modifier::A, Key::L),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0x10, 0, None, 0)),
            ),
            (
                (Modifier::A, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0x10, 0, None, 0)),
            ),
            (
                (Modifier::CA, Key::J),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, -1, None, 0)),
            ),
            (
                (Modifier::CA, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, -1, None, 0)),
            ),
            (
                (Modifier::CA, Key::K),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, 1, None, 0)),
            ),
            (
                (Modifier::CA, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, 1, None, 0)),
            ),
            (
                (Modifier::CA, Key::H),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, -0x10, None, 0)),
            ),
            (
                (Modifier::CA, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, -0x10, None, 0)),
            ),
            (
                (Modifier::CA, Key::L),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, 0x10, None, 0)),
            ),
            (
                (Modifier::CA, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, 0x10, None, 0)),
            ),
            (
                (Modifier::CAS, Key::J),
                UiCommand::Lane(LaneCommand::LaneItemMove(0, -1)),
            ),
            (
                (Modifier::CAS, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::LaneItemMove(0, -1)),
            ),
            (
                (Modifier::CAS, Key::K),
                UiCommand::Lane(LaneCommand::LaneItemMove(0, 1)),
            ),
            (
                (Modifier::CAS, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::LaneItemMove(0, 1)),
            ),
            (
                (Modifier::CAS, Key::H),
                UiCommand::Lane(LaneCommand::LaneItemMove(-1, 0)),
            ),
            (
                (Modifier::CAS, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::LaneItemMove(-1, 0)),
            ),
            (
                (Modifier::CAS, Key::L),
                UiCommand::Lane(LaneCommand::LaneItemMove(1, 0)),
            ),
            (
                (Modifier::CAS, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::LaneItemMove(1, 0)),
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
            (
                (Modifier::S, Key::J),
                UiCommand::Lane(LaneCommand::CursorDownItem),
            ),
            (
                (Modifier::S, Key::ArrowDown),
                UiCommand::Lane(LaneCommand::CursorDownItem),
            ),
            (
                (Modifier::S, Key::K),
                UiCommand::Lane(LaneCommand::CursorUpItem),
            ),
            (
                (Modifier::S, Key::ArrowUp),
                UiCommand::Lane(LaneCommand::CursorUpItem),
            ),
            (
                (Modifier::S, Key::H),
                UiCommand::Lane(LaneCommand::CursorLeftItem),
            ),
            (
                (Modifier::S, Key::ArrowLeft),
                UiCommand::Lane(LaneCommand::CursorLeftItem),
            ),
            (
                (Modifier::S, Key::L),
                UiCommand::Lane(LaneCommand::CursorRightItem),
            ),
            (
                (Modifier::S, Key::ArrowRight),
                UiCommand::Lane(LaneCommand::CursorRightItem),
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
                (Modifier::None, Key::N),
                UiCommand::Lane(LaneCommand::LaneItemUpdate(0, 0, 0, Some(true), 0)),
            ),
            (
                (Modifier::None, Key::Delete),
                UiCommand::Lane(LaneCommand::LaneItemDelete),
            ),
            (
                (Modifier::None, Key::Escape),
                UiCommand::Lane(LaneCommand::SelectClear),
            ),
            (
                (Modifier::None, Key::O),
                UiCommand::Lane(LaneCommand::AutomationParamSelect),
            ),
        ];

        let shortcut_map_pattern = [
            ((Modifier::None, Key::H), UiCommand::PatternCursor(-1, 0)),
            ((Modifier::None, Key::J), UiCommand::PatternCursor(0, 1)),
            ((Modifier::None, Key::K), UiCommand::PatternCursor(0, -1)),
            ((Modifier::None, Key::L), UiCommand::PatternCursor(1, 0)),
            ((Modifier::C, Key::C), UiCommand::PatternCopy),
            ((Modifier::None, Key::D), UiCommand::PatternDup),
            ((Modifier::C, Key::V), UiCommand::PatternPaste),
            ((Modifier::C, Key::X), UiCommand::PatternCut),
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
            (
                (Modifier::None, Key::C),
                UiCommand::Module(ModuleCommand::Sidechain),
            ),
            (
                (Modifier::None, Key::Delete),
                UiCommand::Module(ModuleCommand::Delete),
            ),
            (
                (Modifier::None, Key::Enter),
                UiCommand::Module(ModuleCommand::Open),
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
        let shortcut_map_pattern: HashMap<_, _> = shortcut_map_pattern.into_iter().collect();
        let shortcut_map_module: HashMap<_, _> = shortcut_map_module.into_iter().collect();
        let shortcut_map_mixer: HashMap<_, _> = shortcut_map_mixer.into_iter().collect();

        Self {
            bpm: None,
            dropped_files: Default::default(),
            dropped_files_pre: Default::default(),
            line_play: 0,
            shortcut_map_common,
            shortcut_map_track,
            shortcut_map_lane,
            shortcut_map_pattern,
            shortcut_map_module,
            shortcut_map_mixer,
            stereo_peak_level_states: vec![],
            height_line: 0.0,
            height_mixer: 0.0,
            height_modules: 0.0,
            height_track_header: 0.0,
        }
    }

    pub fn view(
        &mut self,
        gui_context: &egui::Context,
        state: &mut AppState,
        device: &mut Option<Device>,
    ) -> Result<()> {
        // 途中で変わると表示がちらつくので
        self.line_play = state.song_state.line_play;
        // hovered の判定が1フレーム遅れるので。
        self.dropped_files = std::mem::take(&mut self.dropped_files_pre);
        self.dropped_files_pre = gui_context.input(|i| i.raw.dropped_files.clone());

        self.process_shortcut(gui_context, state)?;

        let mut commands = vec![];

        TopBottomPanel::top("Top").show(gui_context, |ui| {
            if false {
                ui.label(format!("{:?}", state.cursor_track));
                ui.label(format!("{:?}", state.selection_track_min));
                ui.label(format!("{:?}", state.selection_track_max));
                ui.label(format!("{:?}", state.song_state.tracks[0]));
                ui.label(format!("{:?}", state.song_state.tracks[1]));
                ui.label(format!("{:?}", state.song_state.tracks[2]));
            }
            ui.horizontal(|ui| -> Result<()> {
                let song_name = format!(
                    "{}{}",
                    if state.song_dirty_p { "*" } else { "" },
                    PathBuf::from(state.song_state.song_file_get().unwrap_or("".to_string()))
                        .file_name()
                        .map(|x| x.to_str())
                        .flatten()
                        .unwrap_or("--")
                );
                ui.heading(song_name);

                if state.song_state.play_p {
                    if ui.button("Stop").clicked() {
                        state.stop()?;
                    }
                } else {
                    if ui.button("Play").clicked() {
                        state.play()?;
                    }
                }

                let mut rec_p = state.song_state.rec_p;
                if ui.toggle_value(&mut rec_p, "REC").clicked() {
                    commands.push(UiCommand::RecToggle);
                }

                let mut bpm = self.bpm.unwrap_or(state.song.bpm);
                let response = ui.add(DragValue::new(&mut bpm).speed(0.1).range(20.0..=999.9));
                if response.has_focus() {
                    self.bpm = Some(bpm);
                } else if self.bpm.is_some() {
                    self.bpm = None;
                    if bpm != state.song.bpm {
                        state.bpm_set(bpm)?;
                    }
                }
                if response.dragged() && bpm != state.song.bpm {
                    self.bpm = None;
                    state.bpm_set(bpm)?;
                }

                ui.label(format!(
                    "{}",
                    play_position_text1(self.line_play, state.song.lpb)
                ));

                ui.label(format!(
                    "{}:{}:{:03}",
                    state.song_state.ms_play / (1000 * 60),
                    (state.song_state.ms_play / 1000) % 60,
                    state.song_state.ms_play % 1000,
                ));

                let mut loop_p = state.song_state.loop_p;
                if ui.toggle_value(&mut loop_p, "Loop").clicked() {
                    state.loop_toggle()?;
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

                ui.label(format!(
                    "{:.3}ms",
                    state.song_state.process_elasped_avg * 1000.0
                ));
                ui.label(format!("{:.3}%", state.song_state.cpu_usage * 100.0));
                ui.label(format!("{:.1}fps", 1.0 / state.elapsed));
                Ok(())
            });
        });

        CentralPanel::default().show(gui_context, |ui: &mut Ui| -> anyhow::Result<()> {
            if state.song_state.play_p && state.follow_p {
                state.cursor_track.line = self.line_play
            }

            let line_range = self.compute_line_range(ui, state);

            with_font_mono(ui, |ui| {
                ui.horizontal(|ui| -> anyhow::Result<()> {
                    let (track_range, lane_start) = self.compute_visible_lane_range(state, ui);
                    if state.pattern_p {
                        self.view_pattern(ui, state, line_range.len(), track_range, lane_start)?;
                    } else {
                        self.view_ruler(state, ui, &line_range)?;

                        self.view_tracks(
                            state,
                            ui,
                            &track_range,
                            lane_start,
                            &line_range,
                            &mut commands,
                        )?;
                    }

                    Ok(())
                });
            });
            Ok(())
        });

        TopBottomPanel::bottom("Bottom").show(gui_context, |ui| {
            ui.label(&state.info);
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
            - self.height_mixer
            - 12.0; // status

        let available_rows = (available_height / self.height_line.max(5.0)).floor() as usize;

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

        if state.song_change_p {
            self.height_line = 0.0;
            self.height_mixer = 0.0;
            self.height_modules = 0.0;
            self.height_track_header = 0.0;
        }

        line_range
    }

    fn compute_visible_lane_range(&self, state: &AppState, ui: &Ui) -> (Range<usize>, usize) {
        let available_width = ui.available_width();
        let flatten_lane_index = state
            .track_lane_to_flatten_lane_index_map
            .get(&(state.cursor_track.track, state.cursor_track.lane))
            .cloned()
            .unwrap_or(0);
        let offset_cursor = state.offset_flatten_lanes[flatten_lane_index];
        let offset_left = (offset_cursor - (available_width / 2.0) + state.width_lane).max(0.0);
        let flatten_lane_index_start = (offset_left / state.width_lane).floor();
        let flatten_lane_index_end =
            (flatten_lane_index_start + (available_width / state.width_lane)).ceil();
        let flatten_lane_index_start =
            (flatten_lane_index_start as usize).clamp(0, state.flatten_lane_index_max);
        let flatten_lane_index_end = (flatten_lane_index_end as usize).clamp(
            flatten_lane_index_start + 1,
            state.flatten_lane_index_max + 1,
        );
        let visiblef_track_start =
            state.flatten_lane_index_to_track_index_vec[flatten_lane_index_start];
        let visiblef_track_end = *state
            .flatten_lane_index_to_track_index_vec
            .get(flatten_lane_index_end)
            .unwrap_or(&state.song.tracks.len());
        let track_range = visiblef_track_start..visiblef_track_end;
        let lane_start = state.flatten_lane_index_to_track_lane_vec[flatten_lane_index_start].1;

        (track_range, lane_start)
    }

    fn lane_item_bg_color(
        &self,
        state: &AppState,
        track_index: usize,
        lane_index: usize,
        line: usize,
    ) -> Color32 {
        let mut bg_color = Color32::BLACK;
        if state.cursor_track.track == track_index
            && state.cursor_track.lane == lane_index
            && state.cursor_track.line == line
            && state.focused_part == FocusedPart::Lane
        {
            bg_color = state.color_cursor();
        } else if line == self.line_play || state.pattern_p && state.in_play_labeled_range_p(line) {
            bg_color = Color32::DARK_GREEN;
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
                    bg_color = Color32::from_rgb(0x40, 0x40, 0xE0);
                }
            }
        } else if line % 0x10 == 0 {
            bg_color = Color32::from_rgb(0x10, 0x10, 0x10);
        } else if line % 4 == 0 {
            bg_color = Color32::from_rgb(0x08, 0x08, 0x08);
        }
        bg_color
    }

    fn lane_item_name(
        &self,
        state: &AppState,
        track_index: usize,
        lane_index: usize,
        line: usize,
    ) -> String {
        match state.song.tracks[track_index].lanes[lane_index].item(line) {
            Some(LaneItem::Note(note)) if note.off => {
                format!("{:<3}    {:02X}", note.note_name(), note.delay)
            }
            Some(LaneItem::Note(note)) => format!(
                "{:<3} {:02X} {:02X}",
                note.note_name(),
                note.velocity as i32,
                note.delay
            ),
            Some(LaneItem::Point(point)) => {
                let param = state.song.tracks[track_index]
                    .automation_params
                    .get(point.automation_params_index)
                    .map(|(module_index, param_id)| {
                        // 8桁あるけど表示スペースがないので下2桁だけ表示
                        format!("{:x}{:X}", module_index, param_id % 0x100)
                    })
                    // point を他のトラックに移動した場合など
                    .unwrap_or("---".to_string());
                format!("{} {:02X} {:02X}", param, point.value, point.delay)
            }
            Some(LaneItem::Label(label)) => format!("'{:<8}", label),

            Some(LaneItem::Call(label)) => format!("^{:<8}", label),

            Some(LaneItem::Ret) => "^        ".to_string(),

            None => "         ".to_string(),
        }
    }

    fn process_shortcut(
        &mut self,
        gui_context: &egui::Context,
        state: &mut AppState,
    ) -> Result<()> {
        let focused = gui_context.memory(|memory| memory.focused());
        if focused.is_some() {
            return Ok(());
        }

        if let Some(key) = shortcut_key(gui_context) {
            let map = match state.focused_part {
                crate::app_state::FocusedPart::Track => &self.shortcut_map_track,
                crate::app_state::FocusedPart::Lane => {
                    if state.pattern_p {
                        &self.shortcut_map_pattern
                    } else {
                        &self.shortcut_map_lane
                    }
                }
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
        if self.stereo_peak_level_states.len() <= track_index {
            self.stereo_peak_level_states
                .resize_with(track_index + 1, Default::default);
        }
        &mut self.stereo_peak_level_states[track_index]
    }

    fn view_mixer(
        &mut self,
        state: &AppState,
        ui: &mut Ui,
        track_index: usize,
        commands: &mut Vec<UiCommand>,
    ) -> Result<()> {
        let inner = ui.vertical(|ui| -> anyhow::Result<()> {
            let track = &state.song.tracks[track_index];
            let peak_level_state = self.stereo_peak_level_state(track_index);
            peak_level_state.update(&state.song_state.tracks[track_index].peaks, state.elapsed);
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

            let mut rec = state.song_state.tracks[track_index].rec_p;
            if ui.toggle_value(&mut rec, "REC").clicked() {
                if rec {
                    commands.push(UiCommand::TrackRecOn(track_index));
                } else {
                    commands.push(UiCommand::TrackRecOff(track_index));
                }
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
                    bg_color: if state.focused_part == FocusedPart::Mixer
                        && track_index == state.cursor_track.track
                    {
                        state.color_cursor()
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
        });
        if self.height_mixer == 0.0 {
            self.height_mixer = inner.response.rect.height();
        }
        Ok(())
    }

    fn view_lines(
        &mut self,
        state: &mut AppState,
        ui: &mut Ui,
        track_range: &Range<usize>,
        lane_start: usize,
        line_range: &Range<usize>,
    ) -> anyhow::Result<()> {
        let mut lane_start = lane_start;
        let font_id = FontId::monospace(12.0);

        for line in line_range.clone() {
            let mut job = LayoutJob::default();
            for track_index in track_range.clone() {
                for lane_index in lane_start..state.song.tracks[track_index].lanes.len() {
                    job.append(
                        " ",
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            background: Color32::from_rgb(0x1b, 0x1b, 0x1b),
                            ..Default::default()
                        },
                    );
                    let text = self.lane_item_name(state, track_index, lane_index, line);
                    let text = format!("{:<9}", text);
                    let color = if state.cursor_track.line == line {
                        Color32::WHITE
                    } else {
                        Color32::GRAY
                    };
                    let bg_color = self.lane_item_bg_color(state, track_index, lane_index, line);
                    job.append(
                        &text,
                        0.0,
                        TextFormat {
                            font_id: font_id.clone(),
                            color,
                            background: bg_color,
                            ..Default::default()
                        },
                    );
                }
                lane_start = 0;
            }
            let label = Label::new(job).truncate();
            if self.height_line == 0.0 {
                let height_before = ui.available_height();
                ui.add(label);
                let height_after = ui.available_height();
                self.height_line = height_before - height_after;
            } else {
                ui.add(label);
            }
        }
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
        let (color, bg_color) = if state.cursor_track.track == track_index
            && state.cursor_module.index == module_index
            && state.focused_part == FocusedPart::Module
        {
            (Color32::LIGHT_GRAY, state.color_cursor())
        } else {
            (Color32::GRAY, Color32::BLACK)
        };
        let label = LabelBuilder::new(ui, &module.name)
            .color(color)
            .bg_color(bg_color)
            .size([DEFAULT_TRACK_WIDTH, 0.0])
            .build();
        if label.clicked() {
            state.send_to_plugin(MainToPlugin::GuiOpen(module.id), Box::new(|_, _| Ok(())))?;
        }
        label.context_menu(|ui: &mut Ui| {
            if ui.button("Delete").clicked() {
                state.plugin_delete((track_index, module_index)).unwrap();
                ui.close_menu();
            }
        });
        Ok(())
    }

    fn view_modules(
        &mut self,
        state: &mut AppState,
        ui: &mut Ui,
        track_index: usize,
    ) -> Result<()> {
        let inner = ui.vertical(|ui| -> anyhow::Result<()> {
            for module_index in 0..state.song.tracks[track_index].modules.len() {
                self.view_module(state, ui, track_index, module_index)?;
            }

            let color = if state.cursor_track.track == track_index
                && state.cursor_module.index == state.song.tracks[track_index].modules.len()
                && state.focused_part == FocusedPart::Module
            {
                state.color_cursor()
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
                if state.cursor_track.track != track_index {
                    state.cursor_track.track = track_index;
                    state.cursor_track.lane = 0;
                }
            }
            Ok(())
        });

        self.height_modules = inner.response.rect.height().max(self.height_modules);

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
                    let color = if line == self.line_play {
                        Color32::DARK_GREEN
                    } else if (state.song_state.loop_start..state.song_state.loop_end)
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
                    let color = if line == self.line_play {
                        Color32::DARK_GREEN
                    } else if (state.song_state.loop_start..state.song_state.loop_end)
                        .contains(&(line * 0x100))
                    {
                        Color32::from_rgb(0x00, 0x30, 0x00)
                    } else {
                        Color32::BLACK
                    };
                    LabelBuilder::new(ui, format!("{:03X}", line))
                        .bg_color(color)
                        .build();
                }
            });
        });
        Ok(())
    }

    fn view_tracks(
        &mut self,
        state: &mut AppState,
        ui: &mut Ui,
        track_range: &Range<usize>,
        lane_start: usize,
        line_range: &Range<usize>,
        mut commands: &mut Vec<UiCommand>,
    ) -> anyhow::Result<()> {
        let mut lane_start = lane_start;
        let inner = ui.vertical(|ui| -> Result<()> {
            self.view_track_head2(state, ui, track_range, lane_start)?;
            self.view_lines(state, ui, track_range, lane_start, line_range)?;
            let mut space = 6.0;
            ui.horizontal(|ui| -> Result<()> {
                for track_index in track_range.clone() {
                    for lane_index in lane_start..state.song.tracks[track_index].lanes.len() {
                        if lane_index == lane_start {
                            ui.add_space(space);
                            space = -2.0;
                            ui.vertical(|ui| -> Result<()> {
                                self.view_mixer(state, ui, track_index, &mut commands)?;
                                self.view_modules(state, ui, track_index)?;
                                Ok(())
                            });
                        } else {
                            ui.add_space(state.width_lane);
                        }
                    }
                    lane_start = 0;
                }
                Ok(())
            });
            Ok(())
        });

        if ui
            .interact(
                inner.response.rect,
                ui.id().with("track"),
                egui::Sense::hover(),
            )
            .hovered()
        {
            for file in self.dropped_files.iter() {
                if let Some(path) = &file.path {
                    if path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map_or(false, |ext| {
                            matches!(ext.to_lowercase().as_str(), "mid" | "midi")
                        })
                    {
                        state.midi_file_read(
                            state.cursor_track.track,
                            state.cursor_track.lane,
                            &path,
                        )?;
                    }
                }
            }
            self.dropped_files.clear();
        }

        Ok(())
    }

    fn view_track_head(
        &mut self,
        state: &mut AppState,
        ui: &mut Ui,
        track_index: usize,
    ) -> Result<()> {
        let height_before_track_header = ui.available_height();
        if state.rename_track_index == Some(track_index) {
            {
                let id = ui.make_persistent_id("track_rename_textedit");
                let edit = TextEdit::singleline(&mut state.rename_buffer).id(id);
                let response = ui.add_sized([DEFAULT_TRACK_WIDTH, 0.0], edit);
                if state.rename_request_focus_p {
                    state.rename_request_focus_p = false;
                    response.request_focus();
                    select_all_text(ui, id, &state.rename_buffer);
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    state.track_rename().unwrap();
                }
            }
        } else {
            let (color, bg_color) = if state.cursor_track.track == track_index
                && state.focused_part == FocusedPart::Track
            {
                (Color32::LIGHT_GRAY, state.color_cursor())
            } else {
                (Color32::GRAY, Color32::BLACK)
            };
            LabelBuilder::new(ui, format!("{:<9}", state.song.tracks[track_index].name))
                .color(color)
                .bg_color(bg_color)
                .build();
        }
        let height_after_track_header = ui.available_height();
        if self.height_track_header == 0.0 {
            self.height_track_header = height_before_track_header - height_after_track_header;
        }
        Ok(())
    }

    fn view_track_head2(
        &mut self,
        state: &mut AppState,
        ui: &mut Ui,
        track_range: &Range<usize>,
        lane_start: usize,
    ) -> Result<()> {
        let height_before_track_header = ui.available_height();

        let font_id = FontId::monospace(12.0);
        let mut job = LayoutJob::default();
        for track_index in track_range.clone() {
            for lane_index in lane_start..state.song.tracks[track_index].lanes.len() {
                job.append(
                    " ",
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        background: Color32::from_rgb(0x1b, 0x1b, 0x1b),
                        ..Default::default()
                    },
                );
                let (color, bg_color) = if state.cursor_track.track == track_index
                    && state.focused_part == FocusedPart::Track
                {
                    (Color32::LIGHT_GRAY, state.color_cursor())
                } else {
                    (Color32::GRAY, Color32::BLACK)
                };
                let text = if lane_index == lane_start {
                    format!("{:<9}", state.song.tracks[track_index].name)
                } else {
                    format!("         ")
                };
                job.append(
                    &text,
                    0.0,
                    TextFormat {
                        font_id: font_id.clone(),
                        color,
                        background: bg_color,
                        ..Default::default()
                    },
                );
            }
        }
        let label = Label::new(job);
        ui.add(label);

        let height_after_track_header = ui.available_height();
        if self.height_track_header == 0.0 {
            self.height_track_header = height_before_track_header - height_after_track_header;
        }
        Ok(())
    }

    fn view_pattern(
        &mut self,
        ui: &mut Ui,
        state: &mut AppState,
        nlines: usize,
        track_range: Range<usize>,
        lane_start: usize,
    ) -> Result<()> {
        let mut lane_start = lane_start;
        let mut commands = vec![];

        with_font_mono(ui, |ui| -> Result<()> {
            ui.horizontal(|ui| -> Result<()> {
                ui.vertical(|ui| -> Result<()> {
                    ui.label(" ");
                    for line in state.labeled_lines.iter().take(nlines) {
                        let text = play_position_text2(*line, state.song.lpb);
                        LabelBuilder::new(ui, text).build();
                    }
                    Ok(())
                });
                ui.vertical(|ui| -> Result<()> {
                    ui.label(" ");
                    for line in state.labeled_lines.iter().take(nlines) {
                        LabelBuilder::new(ui, format!("{:02X}", line)).build();
                    }
                    Ok(())
                });
                for track_index in track_range {
                    ui.vertical(|ui| -> Result<()> {
                        self.view_track_head(state, ui, track_index)?;
                        ui.horizontal(|ui| -> Result<()> {
                            for lane_index in lane_start..state.song.tracks[track_index].lanes.len()
                            {
                                ui.vertical(|ui| -> Result<()> {
                                    for line in state.labeled_lines.iter().take(nlines) {
                                        let line_item = state.song.tracks[track_index].lanes
                                            [lane_index]
                                            .items
                                            .get(line);
                                        let color = match line_item {
                                            Some(LaneItem::Label(_)) => Color32::WHITE,
                                            _ => Color32::DARK_GRAY,
                                        };
                                        let bg_color = self.lane_item_bg_color(
                                            state,
                                            track_index,
                                            lane_index,
                                            *line,
                                        );
                                        let text = if track_index == 0
                                            && lane_index == 0
                                            && state.labeled_lines.last() == Some(line)
                                        {
                                            "(end)    ".to_string()
                                        } else {
                                            self.lane_item_name(
                                                state,
                                                track_index,
                                                lane_index,
                                                *line,
                                            )
                                        };
                                        LabelBuilder::new(ui, text)
                                            .color(color)
                                            .bg_color(bg_color)
                                            .build();
                                    }
                                    Ok(())
                                });
                            }
                            lane_start = 0;
                            Ok(())
                        });
                        self.view_mixer(state, ui, track_index, &mut commands)?;
                        self.view_modules(state, ui, track_index)?;
                        Ok(())
                    });
                }
                Ok(())
            });
            Ok(())
        });

        for command in commands {
            state.run_ui_command(&command)?;
        }

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
    format!("{:03}.{:X}", line / bar + 1, line % bar / lpb + 1)
}
