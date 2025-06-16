use anyhow::Result;
use common::{module::Module, plugin::param::Param};
use eframe::egui::{self, Button, CentralPanel, Key, TextEdit, Ui};

use crate::util::is_subsequence_case_insensitive;

pub struct ParamSelectView {
    focus_p: bool,
    buffer: String,
    queried_modules: Vec<usize>,
    queried_params: Vec<Param>,
    module_index: Option<usize>,
    waite_params_p: bool,
}

impl ParamSelectView {
    pub fn new() -> Self {
        Self {
            focus_p: true,
            buffer: "".to_string(),
            queried_modules: vec![],
            queried_params: vec![],
            module_index: None,
            waite_params_p: false,
        }
    }

    pub fn view(
        &mut self,
        gui_context: &eframe::egui::Context,
        modules: &Vec<Module>,
        params: &Vec<Param>,
    ) -> Result<ReturnState> {
        CentralPanel::default()
            .show(gui_context, |ui: &mut Ui| -> Result<ReturnState> {
                if let Some(module_index) = self.module_index {
                    if !self.waite_params_p {
                        self.waite_params_p = true;
                        return Ok(ReturnState::Params(module_index));
                    }
                    self.view_params(gui_context, ui, params)
                } else {
                    self.view_modules(gui_context, ui, modules)
                }
            })
            .inner
    }

    fn view_modules(
        &mut self,
        gui_context: &eframe::egui::Context,
        ui: &mut Ui,
        modules: &Vec<Module>,
    ) -> Result<ReturnState> {
        let edit = TextEdit::singleline(&mut self.buffer);
        let response = ui.add(edit);
        if response.changed() || self.focus_p {
            self.queried_modules = modules
                .iter()
                .enumerate()
                .filter(|(_module_index, module)| {
                    is_subsequence_case_insensitive(&module.name, &self.buffer)
                })
                .map(|(module_index, _module)| module_index)
                .collect::<Vec<_>>()
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            self.module_index = Some(self.queried_modules[0]);
            return Ok(ReturnState::Continue);
        }
        if self.focus_p {
            self.focus_p = false;
            gui_context.memory_mut(|x| x.request_focus(response.id));
        }

        for module_index in self.queried_modules.iter() {
            let button =
                Button::new(&modules[*module_index].name).wrap_mode(egui::TextWrapMode::Extend);
            if ui.add(button).clicked() {
                self.module_index = Some(*module_index);
                return Ok(ReturnState::Continue);
            }
        }

        ui.separator();

        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
            return Ok(ReturnState::Cancel);
        }

        Ok(ReturnState::Continue)
    }

    fn view_params(
        &mut self,
        gui_context: &eframe::egui::Context,
        ui: &mut Ui,
        params: &Vec<Param>,
    ) -> Result<ReturnState> {
        let edit = TextEdit::singleline(&mut self.buffer);
        let response = ui.add(edit);
        if response.changed() || self.focus_p {
            self.queried_params = params
                .iter()
                .filter(|x| is_subsequence_case_insensitive(&x.name, &self.buffer))
                .map(|x| x.clone())
                .collect::<Vec<_>>()
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            return Ok(ReturnState::Selected(
                self.module_index.unwrap(),
                self.queried_params[0].clone(),
            ));
        }
        if self.focus_p {
            self.focus_p = false;
            gui_context.memory_mut(|x| x.request_focus(response.id));
        }

        for item in self.queried_params.iter() {
            let button = Button::new(&item.name).wrap_mode(egui::TextWrapMode::Extend);
            if ui.add(button).clicked() {
                return Ok(ReturnState::Selected(
                    self.module_index.unwrap(),
                    item.clone(),
                ));
            }
        }

        ui.separator();

        if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
            return Ok(ReturnState::Cancel);
        }

        Ok(ReturnState::Continue)
    }
}

pub enum ReturnState {
    Selected(usize, Param),
    Params(usize),
    Continue,
    Cancel,
}
