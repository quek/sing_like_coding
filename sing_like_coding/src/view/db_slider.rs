use eframe::egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2, Widget};

use common::dsp::{db_from_norm, db_to_norm};

pub struct DbSlider<'a> {
    pub db_value: &'a mut f32,
    pub min_db: f32,
    pub max_db: f32,
    pub height: f32,
}

impl<'a> Widget for DbSlider<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let width = 12.0;
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, self.height), Sense::click_and_drag());
        let painter = ui.painter_at(rect);

        let norm = db_to_norm(*self.db_value, self.min_db, self.max_db);
        let mut norm_value = 1.0 - norm; // 上が最大になるように反転

        if response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 1.0);
                norm_value = t;
                *self.db_value = db_from_norm(1.0 - norm_value, self.min_db, self.max_db);
            }
        }

        // 背景
        painter.rect_filled(rect, 0.0, Color32::BLACK);

        // スライダーのノブ
        let knob_y = rect.top() + rect.height() * norm_value;
        let knob_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, knob_y),
            Vec2::new(rect.width(), 5.0),
        );
        painter.rect_filled(knob_rect, 0.0, Color32::LIGHT_GRAY);

        // メモリ描画
        let steps = [-60, -36, -24, -18, -12, -6, 0];
        for &db in &steps {
            if db as f32 >= self.min_db && db as f32 <= self.max_db {
                let y =
                    rect.bottom() - rect.height() * db_to_norm(db as f32, self.min_db, self.max_db);
                painter.line_segment(
                    [
                        Pos2::new(rect.left() + 2.0, y),
                        Pos2::new(rect.left() + 8.0, y),
                    ],
                    (1.0, Color32::GRAY),
                );
            }
        }

        response
    }
}
