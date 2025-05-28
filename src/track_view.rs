use std::{
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    thread,
};

use eframe::egui::Ui;

use crate::{note::note_name_to_midi, song::SongCommand};

#[derive(Debug)]
pub enum ViewCommand {
    Play,
    Stop,
    StateTrack(usize),
    Note(usize, i16),
}

pub struct TrackView {
    line_buffers: Vec<String>,
    view_sender: Sender<ViewCommand>,
    track_name: String,
    gui_context: Option<eframe::egui::Context>,
}

impl TrackView {
    pub fn new(view_sender: Sender<ViewCommand>) -> Self {
        Self {
            line_buffers: vec![],
            view_sender,
            track_name: "".to_string(),
            gui_context: None,
        }
    }

    pub fn start_listener(view: Arc<Mutex<Self>>, receiver: Receiver<SongCommand>) {
        log::debug!("TrackView::start_listener");
        thread::spawn(move || {
            while let Ok(command) = receiver.recv() {
                match command {
                    SongCommand::Track => (),
                    SongCommand::Note => (),
                    SongCommand::StateTrack(name, nlines, notes) => {
                        let mut view = view.lock().unwrap();
                        view.track_name = name;
                        view.line_buffers.clear();
                        for line in 0..nlines {
                            if let Some(note) = notes.iter().find(|note| note.line == line) {
                                view.line_buffers.push(note.note_name());
                            } else {
                                view.line_buffers.push("".to_string());
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn view(&mut self, ui: &mut Ui, gui_context: &eframe::egui::Context) {
        if self.gui_context.is_none() {
            self.gui_context = Some(gui_context.clone());
        }
        ui.heading(&self.track_name);
        for line in 0..self.line_buffers.len() {
            ui.horizontal(|ui| {
                ui.label(format!("{:02X}", line));
                if ui
                    .text_edit_singleline(&mut self.line_buffers[line])
                    .changed()
                {
                    note_name_to_midi(&self.line_buffers[line]).map(|key| {
                        self.view_sender.send(ViewCommand::Note(line, key)).unwrap();
                    });
                }
            });
        }
    }
}
