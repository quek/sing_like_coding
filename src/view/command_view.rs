use anyhow::Result;
use eframe::egui::{CentralPanel, Key, TextEdit, Ui};

use crate::{command::Command, commander::Commander};

use super::{main_view::Route, view_state::ViewState};

pub struct CommandView {
    focus_p: bool,
    buffer: String,
    commander: Commander,
    commands: Vec<Box<dyn Command>>,
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
            if response.changed() {
                self.commands = self.commander.query(&self.buffer);
                log::debug!("a commands.len {}", self.commands.len());
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                log::debug!("b commands.len {}", self.commands.len());
                if self.commands.len() == 1 {
                    log::debug!("enter");
                    self.commands[0].call()?;
                }
            }
            if self.focus_p {
                self.focus_p = false;
                gui_context.memory_mut(|x| x.request_focus(response.id));
            }

            if ui.button("Cancel").clicked() {
                self.focus_p = true;
                state.route = Route::Main;
            }

            Ok(())
        });
        Ok(())
    }
}
