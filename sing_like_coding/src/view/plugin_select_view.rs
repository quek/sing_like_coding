use anyhow::Result;
use common::{clap_manager::ClapManager, plugin::description::Description};
use eframe::egui::{self, Button, CentralPanel, Key, TextEdit, Ui};

use crate::util::is_subsequence_case_insensitive;

pub struct PluginSelectView {
    focus_p: bool,
    buffer: String,
    descriptions: Vec<Description>,
    quried_items: Vec<Description>,
}

impl PluginSelectView {
    pub fn new() -> Self {
        let mut clap_manager = ClapManager::new();
        clap_manager.load().unwrap();
        let descriptions = clap_manager.descriptions;
        Self {
            focus_p: true,
            buffer: "".to_string(),
            descriptions,
            quried_items: vec![],
        }
    }

    pub fn view(&mut self, gui_context: &eframe::egui::Context) -> Result<ReturnState> {
        CentralPanel::default()
            .show(gui_context, |ui: &mut Ui| -> Result<ReturnState> {
                let edit = TextEdit::singleline(&mut self.buffer);
                let response = ui.add(edit);
                if response.changed() || self.focus_p {
                    self.quried_items = self
                        .descriptions
                        .iter_mut()
                        .filter(|x| is_subsequence_case_insensitive(&x.name, &self.buffer))
                        .map(|x| x.clone())
                        .collect::<Vec<_>>()
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    return Ok(ReturnState::Selected(self.quried_items[0].clone()));
                }
                if self.focus_p {
                    self.focus_p = false;
                    gui_context.memory_mut(|x| x.request_focus(response.id));
                }

                let mut selected = None;
                ui.horizontal_wrapped(|ui| {
                    for item in self.quried_items.iter() {
                        let button = Button::new(&item.name).wrap_mode(egui::TextWrapMode::Extend);
                        if ui.add(button).clicked() {
                            selected = Some(item.clone());
                        }
                    }
                });
                if let Some(item) = selected {
                    return Ok(ReturnState::Selected(item));
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

pub enum ReturnState {
    Selected(Description),
    Continue,
    Cancel,
}
