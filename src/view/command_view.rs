use std::sync::{Arc, Mutex};

use anyhow::Result;
use eframe::egui::{self, Button, CentralPanel, Key, TextEdit, Ui};

use crate::{command::Command, commander::Commander};

use super::{main_view::Route, view_state::ViewState};

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
        state: &mut ViewState,
    ) -> Result<()> {
        CentralPanel::default().show(gui_context, |ui: &mut Ui| -> Result<()> {
            let edit = TextEdit::singleline(&mut self.buffer);
            let response = ui.add(edit);
            if response.changed() || (self.focus_p && !self.buffer.is_empty()) {
                self.commands = self.commander.query(&self.buffer);
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.commands[0].lock().unwrap().call()?;
                self.close(state);
                return Ok(());
            }
            if self.focus_p {
                self.focus_p = false;
                gui_context.memory_mut(|x| x.request_focus(response.id));
            }

            let mut called = false;
            for command in self.commands.iter() {
                let mut command = command.lock().unwrap();
                let button = Button::new(command.name()).wrap_mode(egui::TextWrapMode::Extend);
                if ui.add(button).clicked() {
                    command.call()?;
                    called = true;
                }
            }
            if called {
                self.close(state);
                return Ok(());
            }

            ui.separator();

            if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                self.close(state);
            }

            Ok(())
        });

        Ok(())
    }

    fn close(&mut self, state: &mut ViewState) {
        self.focus_p = true;
        self.commands.clear();
        state.route = Route::Main;
    }
}
