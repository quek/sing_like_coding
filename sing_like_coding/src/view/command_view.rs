use std::sync::{Arc, Mutex};

use anyhow::Result;
use eframe::egui::{self, Button, CentralPanel, Key, TextEdit, Ui};

use crate::{app_state::AppState, command::Command, commander::Commander};

use super::{root_view::Route, util::select_all_text};

pub struct CommandView {
    focus_p: bool,
    buffer: String,
    commander: Commander,
    commands: Vec<Arc<Mutex<dyn Command>>>,
}

impl CommandView {
    pub fn new() -> Self {
        Self {
            focus_p: true,
            buffer: "".to_string(),
            commander: Commander::new(),
            commands: vec![],
        }
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut AppState,
    ) -> Result<()> {
        CentralPanel::default().show(gui_context, |ui: &mut Ui| -> Result<()> {
            let edit = TextEdit::singleline(&mut self.buffer);
            let response = ui.add(edit);
            if response.changed() || self.focus_p {
                self.commands = self.commander.query(&self.buffer);
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                if let Some(command) = self.commands.first() {
                    state.route = Route::Track;
                    command.lock().unwrap().call(state)?;
                    self.close();
                    return Ok(());
                }
            }
            if self.focus_p {
                self.focus_p = false;
                gui_context.memory_mut(|x| {
                    x.request_focus(response.id);
                });
                select_all_text(ui, response.id, &self.buffer);
            }

            let mut selected = None;
            ui.horizontal_wrapped(|ui| {
                for command in self.commands.iter().cloned() {
                    let name = command.lock().unwrap().name().to_string();
                    let button = Button::new(name).wrap_mode(egui::TextWrapMode::Extend);
                    if ui.add(button).clicked() {
                        selected = Some(command);
                    }
                }
            });
            if let Some(command) = selected {
                state.route = Route::Track;
                command.lock().unwrap().call(state)?;
                self.close();
                return Ok(());
            }

            ui.separator();

            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                state.route = Route::Track;
                self.close();
            }

            Ok(())
        });

        Ok(())
    }

    fn close(&mut self) {
        self.focus_p = true;
        self.commands.clear();
    }
}
