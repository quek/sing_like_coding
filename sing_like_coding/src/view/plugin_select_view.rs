use anyhow::Result;
use common::plugin::description::Description;
use eframe::egui::{self, Button, CentralPanel, Key, TextEdit, Ui};

use crate::util::is_subsequence_case_insensitive;

pub struct PluginSelectView {
    focus_p: bool,
    buffer: String,
    all_items: Vec<Description>,
    quried_items: Vec<Description>,
}

impl PluginSelectView {
    pub fn new(all_items: Vec<Description>) -> Self {
        Self {
            focus_p: true,
            buffer: "".to_string(),
            all_items,
            quried_items: vec![],
        }
    }

    pub fn view(&mut self, gui_context: &eframe::egui::Context) -> Result<Option<Description>> {
        CentralPanel::default()
            .show(gui_context, |ui: &mut Ui| -> Result<Option<Description>> {
                let edit = TextEdit::singleline(&mut self.buffer);
                let response = ui.add(edit);
                if response.changed() || self.focus_p {
                    self.quried_items = self
                        .all_items
                        .iter_mut()
                        .filter(|x| is_subsequence_case_insensitive(&x.name, &self.buffer))
                        .map(|x| x.clone())
                        .collect::<Vec<_>>()
                }
                if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    self.close();
                    return Ok(Some(self.quried_items[0].clone()));
                }
                if self.focus_p {
                    self.focus_p = false;
                    gui_context.memory_mut(|x| x.request_focus(response.id));
                }

                let mut selected = None;
                for item in self.quried_items.iter() {
                    let button = Button::new(&item.name).wrap_mode(egui::TextWrapMode::Extend);
                    if ui.add(button).clicked() {
                        selected = Some(item.clone());
                    }
                }
                if selected.is_some() {
                    self.close();
                    return Ok(selected);
                }

                ui.separator();

                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                    self.close();
                }

                Ok(None)
            })
            .inner
    }

    fn close(&mut self) {
        self.focus_p = true;
    }
}
