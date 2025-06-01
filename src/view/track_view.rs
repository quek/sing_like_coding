use std::sync::{Arc, Mutex};

use anyhow::Result;
use eframe::egui::{CentralPanel, Color32, Frame, Key, TopBottomPanel, Ui};

use crate::{
    device::Device,
    singer::{Singer, SingerMsg},
};

use super::view_state::ViewState;

pub struct TrackView {}

impl TrackView {
    pub fn new() -> Self {
        Self {}
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut ViewState,
        device: &mut Option<Device>,
        singer: &Arc<Mutex<Singer>>,
    ) -> Result<()> {
        self.process_shortcut(gui_context, state)?;

        TopBottomPanel::top("Top").show(gui_context, |ui| {
            ui.heading("Sing Like Coding");
        });

        CentralPanel::default().show(gui_context, |ui: &mut Ui| {
            ui.horizontal(|ui| {
                if ui.button("device start").clicked() {
                    device.as_mut().unwrap().start(singer.clone()).unwrap();
                }
                if ui.button("device stop").clicked() {
                    device.as_mut().unwrap().stop().unwrap();
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label(format!("line {}", state.song_state.line_play));
                if ui.button("Play").clicked() {
                    state.view_sender.send(SingerMsg::Play).unwrap();
                }
                if ui.button("Stop").clicked() {
                    state.view_sender.send(SingerMsg::Stop).unwrap();
                }

                if ui.button("Load Surge XT").clicked() {
                    let path = "c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap"
                        .to_string();
                    let track_index = state.song.tracks.len() - 1;
                    state
                        .view_sender
                        .send(SingerMsg::PluginLoad(track_index, path, 0))
                        .unwrap();
                }

                if ui.button("Load VCV Rack 2").clicked() {
                    let path = "c:/Program Files/Common Files/CLAP/VCV Rack 2.clap".to_string();
                    let track_index = state.song.tracks.len() - 1;
                    state
                        .view_sender
                        .send(SingerMsg::PluginLoad(track_index, path, 0))
                        .unwrap();
                }

                if ui.button("Load TyrellN6").clicked() {
                    let path = "c:/Program Files/Common Files/CLAP/u-he/TyrellN6.clap".to_string();
                    let track_index = state.song.tracks.len() - 1;
                    state
                        .view_sender
                        .send(SingerMsg::PluginLoad(track_index, path, 0))
                        .unwrap();
                }

                if ui.button("Load Zebralette3").clicked() {
                    let path =
                        "c:/Program Files/Common Files/CLAP/u-he/Zebralette3.clap".to_string();
                    let track_index = state.song.tracks.len() - 1;
                    state
                        .view_sender
                        .send(SingerMsg::PluginLoad(track_index, path, 0))
                        .unwrap();
                }

                if ui.button("Open").clicked() {
                    // main thread で処理しないといけないので、send せずに実装
                    log::debug!("Open before lock");
                    let mut singer = singer.lock().unwrap();
                    log::debug!("Open after lock");
                    let track_index = state.song.tracks.len() - 1;
                    let plugin = &mut singer.plugins[track_index][0];
                    log::debug!("Open plugin");
                    plugin.gui_open().unwrap();
                    log::debug!("did gui_open");
                }
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Note On").clicked() {
                    let track_index = 0;
                    let key = 63;
                    let channel = 0;
                    let velocity = 100.0;
                    let time = 0;
                    state
                        .view_sender
                        .send(SingerMsg::NoteOn(track_index, key, channel, velocity, time))
                        .unwrap();
                }

                if ui.button("Note Off").clicked() {
                    let track_index = 0;
                    let key = 63;
                    let channel = 0;
                    let velocity = 0.0;
                    let time = 0;
                    state
                        .view_sender
                        .send(SingerMsg::NoteOff(
                            track_index,
                            key,
                            channel,
                            velocity,
                            time,
                        ))
                        .unwrap();
                }
            });

            ui.separator();

            let nlines = state.song.nlines;
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(format!("{:02X}", nlines));
                    for line in 0..nlines {
                        Frame::NONE
                            .fill(if line == state.song_state.line_play % 0x0F {
                                Color32::DARK_GREEN
                            } else {
                                Color32::BLACK
                            })
                            .show(ui, |ui| {
                                ui.label(format!("{:02X}", line));
                            });
                    }
                });
                for (track_index, (track, line_buffer)) in state
                    .song
                    .tracks
                    .iter()
                    .zip(state.line_buffers.iter_mut())
                    .enumerate()
                {
                    ui.vertical(|ui| {
                        Frame::NONE
                            .fill(if state.selected_tracks.contains(&track_index) {
                                Color32::GREEN
                            } else {
                                Color32::BLACK
                            })
                            .show(ui, |ui| {
                                ui.label(&track.name);
                            });
                        for line in 0..nlines {
                            let color =
                                if state.cursor_track == track_index && state.cursor_line == line {
                                    Color32::YELLOW
                                } else if line == state.song_state.line_play % 0x0f {
                                    Color32::DARK_GREEN
                                } else if state.selected_cells.contains(&(track_index, line)) {
                                    Color32::LIGHT_BLUE
                                } else {
                                    Color32::BLACK
                                };
                            Frame::NONE.fill(color).show(ui, |ui| {
                                let mut text = line_buffer[line].as_str();
                                if text.is_empty() {
                                    text = "----";
                                }
                                ui.label(text);
                            });
                        }
                    });
                }
            });
            Ok::<(), anyhow::Error>(())
        });

        Ok(())
    }

    fn process_shortcut(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut ViewState,
    ) -> Result<()> {
        let input = gui_context.input(|i| i.clone());
        let focused = gui_context.memory(|memory| memory.focused());
        if focused.is_some() {
            return Ok(());
        }

        if input.modifiers.ctrl {
            if input.key_pressed(Key::J) {
                note_update(-1, state);
            } else if input.key_pressed(Key::K) {
                note_update(1, state);
            } else if input.key_pressed(Key::H) {
                note_update(-12, state);
            } else if input.key_pressed(Key::L) {
                note_update(12, state);
            }
        } else if input.key_pressed(Key::J) {
            if state.cursor_line + 1 == state.song.nlines {
                state.cursor_line = 0;
            } else {
                state.cursor_line += 1;
            }
        } else if input.key_pressed(Key::K) {
            if state.cursor_line == 0 {
                state.cursor_line = state.song.nlines - 1;
            } else {
                state.cursor_line -= 1;
            }
        } else if input.key_pressed(Key::H) {
            if state.cursor_track == 0 {
                state.cursor_track = state.song.tracks.len() - 1;
            } else {
                state.cursor_track -= 1;
            }
        } else if input.key_pressed(Key::L) {
            if state.cursor_track + 1 == state.song.tracks.len() {
                state.cursor_track = 0;
            } else {
                state.cursor_track += 1;
            }
        }

        Ok(())
    }
}

fn note_update(key_delta: i16, state: &mut ViewState) {
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
