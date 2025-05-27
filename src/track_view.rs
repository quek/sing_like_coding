use eframe::egui::Ui;

use crate::track::Track;

pub struct TrackView {}

impl TrackView {
    pub fn view(ui: &mut Ui, track: &Track) {
        ui.heading("Track 01");
    }
}
