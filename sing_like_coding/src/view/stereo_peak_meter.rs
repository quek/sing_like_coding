use std::time::Instant;

use common::dsp::db_to_norm;
use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Ui, Vec2, Widget,
};

pub const DB_MIN: f32 = -60.0;
pub const DB_MAX: f32 = 6.0;

#[derive(Default)]
pub struct StereoPeakLevelState {
    pub left: PeakLevelState,
    pub right: PeakLevelState,
}

impl StereoPeakLevelState {
    pub fn update(&mut self, peaks: &[f32]) {
        self.left.update(peaks[0]);
        self.right.update(peaks[1]);
    }
}

pub struct PeakLevelState {
    pub current_db: f32,
    pub hold_db: f32,
    pub hold_timer: f32,
    pub now: Instant,
}

impl Default for PeakLevelState {
    fn default() -> Self {
        Self {
            current_db: DB_MIN,
            hold_db: DB_MIN,
            hold_timer: 0.0,
            now: Instant::now(),
        }
    }
}

impl PeakLevelState {
    pub fn update(&mut self, new_db: f32) {
        let dt = self.now.elapsed().as_secs_f32();
        let hold_time = 1.0;
        let fall_rate = 20.0;

        self.current_db = new_db;

        if new_db >= self.hold_db {
            self.hold_db = new_db;
            self.hold_timer = 0.0;
        } else if self.hold_db > DB_MIN {
            self.hold_timer += dt;
            if self.hold_timer > hold_time {
                self.hold_db = (self.hold_db - fall_rate * dt).max(new_db).max(DB_MIN);
            }
        }
        self.now = Instant::now();
    }
}

pub struct StereoPeakMeter<'a> {
    pub peak_level_state: &'a StereoPeakLevelState,
    pub min_db: f32,
    pub max_db: f32,
    pub show_scale: bool,
    pub height: f32,
}

impl<'a> Widget for StereoPeakMeter<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let meter_width = 6.0;
        let spacing = 4.0;
        let scale_width = if self.show_scale { 14.0 } else { 0.0 };
        let total_width = meter_width * 2.0 + spacing + scale_width;

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(total_width, self.height), Sense::hover());
        let painter = ui.painter_at(rect);

        let left_rect = Rect::from_min_size(
            rect.min + Vec2::new(scale_width, 0.0),
            Vec2::new(meter_width, self.height),
        );
        let right_rect = Rect::from_min_size(
            Pos2::new(left_rect.max.x + spacing, rect.min.y),
            Vec2::new(meter_width, self.height),
        );

        if self.show_scale {
            let scale_rect = Rect::from_min_size(rect.min, Vec2::new(scale_width, self.height));
            draw_db_scale(&painter, scale_rect, self.min_db, self.max_db);
        }

        draw_meter(
            &painter,
            left_rect,
            &self.peak_level_state.left,
            self.min_db,
            self.max_db,
        );
        draw_meter(
            &painter,
            right_rect,
            &self.peak_level_state.right,
            self.min_db,
            self.max_db,
        );

        response
    }
}

fn draw_meter(painter: &Painter, rect: Rect, level: &PeakLevelState, min_db: f32, max_db: f32) {
    let norm = |db: f32| db_to_norm(db, min_db, max_db);

    let curr_h = rect.height() * norm(level.current_db);
    let hold_h = rect.height() * norm(level.hold_db);

    // 背景
    painter.rect_filled(rect, 2.0, Color32::BLACK);

    // メーターバー（緑）
    let bar = Rect::from_min_max(
        Pos2::new(rect.left(), rect.bottom() - curr_h),
        Pos2::new(rect.right(), rect.bottom()),
    );
    painter.rect_filled(bar, 1.0, Color32::GREEN);

    // ホールドピークライン（赤）
    let y = rect.bottom() - hold_h;
    painter.line_segment(
        [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
        (1.5, Color32::RED),
    );
}

fn draw_db_scale(painter: &Painter, rect: Rect, min_db: f32, max_db: f32) {
    let steps = [-60, -36, -24, -18, -12, -6, 0];
    for &db in &steps {
        if db < min_db as i32 || db > max_db as i32 {
            continue;
        }
        let norm = db_to_norm(db as f32, min_db, max_db);
        let y = rect.bottom() - rect.height() * norm;
        let x = rect.right();

        painter.text(
            Pos2::new(x - 2.0, y - 6.0),
            Align2::RIGHT_CENTER,
            format!("{}", db.abs()),
            FontId::monospace(10.0),
            Color32::GRAY,
        );
    }
}
