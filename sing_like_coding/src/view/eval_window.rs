use anyhow::Result;
use eframe::egui::{Align2, Context, Key, Window};

use crate::app_state::AppState;

pub struct EvalWindow {
    buffer: String,
}

impl EvalWindow {
    pub fn new() -> Self {
        Self {
            buffer: Default::default(),
        }
    }

    pub fn view(&mut self, ctx: &Context, state: &mut AppState) -> Result<()> {
        Window::new("eval")
            .collapsible(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| -> Result<()> {
                ui.label("You have unsaved changes.");

                let edit = ui.text_edit_singleline(&mut self.buffer);
                edit.request_focus();
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    state.eval_window_open_p = false;
                    state.eval(&self.buffer)?;
                    self.buffer.clear();
                }

                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    state.eval_window_open_p = true;
                }
                Ok(())
            });
        Ok(())
    }
}
