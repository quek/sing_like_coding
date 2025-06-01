use anyhow::Result;
use eframe::egui::{CentralPanel, Ui};

use super::{main_view::Route, view_state::ViewState};

pub struct CommandView {
    focus_p: bool,
    buffer: String,
}

impl CommandView {
    pub fn new() -> Self {
        Self {
            focus_p: true,
            buffer: "".to_string(),
        }
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        state: &mut ViewState,
    ) -> Result<()> {
        CentralPanel::default().show(gui_context, |ui: &mut Ui| {
            let edit = ui.text_edit_singleline(&mut self.buffer);
            if edit.changed() {
                dbg!(&self.buffer);
            }
            if self.focus_p {
                self.focus_p = false;
                gui_context.memory_mut(|x| x.request_focus(edit.id));
            }

            if ui.button("Cancel").clicked() {
                self.focus_p = true;
                state.route = Route::Main;
            }
        });
        Ok(())
    }
}
