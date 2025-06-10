use eframe::egui::{
    Align2, Color32, FontId, Painter, Pos2, Rect, Response, Sense, Ui, Vec2, Widget,
};

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

#[derive(Default)]
pub struct PeakLevelState {
    pub current_db: f32,
    pub hold_db: f32,
    pub hold_timer: f32,
}

impl PeakLevelState {
    pub fn update(&mut self, new_db: f32) {
        let dt = 1.0 / 60.0;
        let hold_time = 1.0;
        let fall_rate = 20.0;

        self.current_db = new_db;

        if new_db >= self.hold_db {
            self.hold_db = new_db;
            self.hold_timer = 0.0;
        } else {
            self.hold_timer += dt;
            if self.hold_timer > hold_time {
                self.hold_db = (self.hold_db - fall_rate * dt).max(new_db);
            }
        }
    }
}

pub struct StereoPeakMeter<'a> {
    pub peak_level_state: &'a StereoPeakLevelState,
    pub min_db: f32,
    pub max_db: f32,
    pub show_scale: bool,
}

impl<'a> Widget for StereoPeakMeter<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let meter_width = 12.0;
        let spacing = 8.0;
        let height = 120.0;
        let scale_width = if self.show_scale { 20.0 } else { 0.0 };
        let total_width = meter_width * 2.0 + spacing + scale_width;

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(total_width, height), Sense::hover());
        let painter = ui.painter_at(rect);

        let left_rect = Rect::from_min_size(
            rect.min + Vec2::new(scale_width, 0.0),
            Vec2::new(meter_width, height),
        );
        let right_rect = Rect::from_min_size(
            Pos2::new(left_rect.max.x + spacing, rect.min.y),
            Vec2::new(meter_width, height),
        );

        if self.show_scale {
            let scale_rect = Rect::from_min_size(rect.min, Vec2::new(scale_width, height));
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

fn normalize_db(db: f32, min_db: f32, max_db: f32) -> f32 {
    let db = db.clamp(min_db, max_db);
    // 0.0 = min_db, 1.0 = max_db の範囲に正規化
    let t = (db - min_db) / (max_db - min_db);
    curved(t, 1.5)
}

// 非線形カーブ。index < 1.0 で下側を拡大（-6dB付近の視認性向上）
fn curved(t: f32, index: f32) -> f32 {
    t.powf(index)
}

fn draw_meter(painter: &Painter, rect: Rect, level: &PeakLevelState, min_db: f32, max_db: f32) {
    let norm = |db: f32| normalize_db(db, min_db, max_db);

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
        let norm = normalize_db(db as f32, min_db, max_db);
        let y = rect.bottom() - rect.height() * norm;
        let x = rect.right();

        painter.text(
            Pos2::new(x - 2.0, y - 6.0),
            Align2::RIGHT_CENTER,
            format!("{db}"),
            FontId::monospace(10.0),
            Color32::GRAY,
        );
    }
}
