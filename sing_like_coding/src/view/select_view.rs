use anyhow::Result;
use eframe::egui::{self, Button, CentralPanel, Key, TextEdit, Ui};

use crate::util::is_subsequence_case_insensitive;

pub trait SelectItem: Clone {
    fn name(&self) -> &str;
}

#[derive(Debug)]
pub enum ReturnState<T> {
    Selected(T),
    Continue,
    Cancel,
}

pub struct SelectView<T: SelectItem> {
    focus_p: bool,
    buffer: String,
    items: Vec<T>,
    quried_items: Vec<T>,
}

impl<T: SelectItem> SelectView<T> {
    pub fn new(items: Vec<T>) -> Self {
        Self {
            focus_p: true,
            buffer: String::new(),
            items,
            quried_items: vec![],
        }
    }

    pub fn view(&mut self, gui_context: &eframe::egui::Context) -> Result<ReturnState<T>> {
        CentralPanel::default()
            .show(gui_context, |ui: &mut Ui| -> Result<ReturnState<T>> {
                let edit = TextEdit::singleline(&mut self.buffer);
                let response = ui.add(edit);

                if response.changed() || self.focus_p {
                    self.quried_items = self
                        .items
                        .iter()
                        .filter(|x| is_subsequence_case_insensitive(x.name(), &self.buffer))
                        .cloned()
                        .collect();
                }

                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    if let Some(selected) = self.quried_items.get(0).cloned() {
                        return Ok(ReturnState::Selected(selected));
                    }
                }

                if self.focus_p {
                    self.focus_p = false;
                    gui_context.memory_mut(|mem| mem.request_focus(response.id));
                }

                for item in &self.quried_items {
                    let button = Button::new(item.name()).wrap_mode(egui::TextWrapMode::Extend);
                    if ui.add(button).clicked() {
                        return Ok(ReturnState::Selected(item.clone()));
                    }
                }

                ui.separator();

                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    return Ok(ReturnState::Cancel);
                }

                Ok(ReturnState::Continue)
            })
            .inner
    }
}
