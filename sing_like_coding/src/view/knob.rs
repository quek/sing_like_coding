use std::f32::consts::PI;

use eframe::egui::{Align2, Color32, Pos2, Response, Sense, TextStyle, Ui, Vec2, Widget};

pub struct Knob<'a> {
    pub value: &'a mut f32, // 0.0 ～ 1.0
}

impl<'a> Widget for Knob<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let desired_size = Vec2::new(34.0, 48.0);
        let (rect, response) = ui.allocate_exact_size(desired_size, Sense::click_and_drag());

        let center = rect.center() - Vec2::new(0.0, (desired_size.y - desired_size.x) / 2.0);
        let radius = rect.width().min(rect.height()) * 0.5 - 4.0;

        // マウスドラッグ処理
        if response.dragged() {
            let delta = response.drag_delta().y; // 垂直ドラッグで調整
            *self.value -= delta * 0.005;
            *self.value = self.value.clamp(0.0, 1.0);
            ui.ctx().request_repaint(); // 再描画要求
        }

        let painter = ui.painter();

        // ノブの外円
        painter.circle_stroke(center, radius, (2.0, Color32::GRAY));

        // 中央マーカー（ノブの正面方向）
        let angle_center = std::f32::consts::FRAC_PI_2 * 3.0; // 上方向
        let marker_len = radius - 0.0;
        let marker_pos = Pos2 {
            x: center.x + marker_len * angle_center.cos(),
            y: center.y + marker_len * angle_center.sin(),
        };
        painter.line_segment([center, marker_pos], (1.0, Color32::DARK_GRAY));

        // パン値に応じたノブの針
        let angle = angle_center + (*self.value * 1.6 - 0.8) * PI;
        let needle_len = radius - 0.0;
        let needle_pos = Pos2 {
            x: center.x + needle_len * angle.cos(),
            y: center.y + needle_len * angle.sin(),
        };
        painter.line_segment([center, needle_pos], (2.0, Color32::WHITE));

        // 数値表示（下部）
        let text = if (*self.value - 0.5).abs() < 0.001 {
            "C".to_string()
        } else if *self.value < 0.5 {
            format!("L{}", ((0.5 - *self.value) * 200.0).round() as i32)
        } else {
            format!("R{}", ((*self.value - 0.5) * 200.0).round() as i32)
        };
        painter.text(
            Pos2::new(center.x, rect.bottom() - 12.0),
            Align2::CENTER_TOP,
            text,
            TextStyle::Small.resolve(ui.style()),
            Color32::LIGHT_GRAY,
        );

        response
    }
}
