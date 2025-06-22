use eframe::egui::{
    text::{CCursor, CCursorRange},
    text_edit::TextEditState,
    Color32, Frame, Id, Response, RichText, Ui, Vec2,
};

pub struct LabelBuilder<'a> {
    ui: &'a mut Ui,
    text: String,
    color: Color32,
    bg_color: Color32,
    size: Option<Vec2>,
}

impl<'a> LabelBuilder<'a> {
    pub fn new(ui: &'a mut Ui, text: impl Into<String>) -> Self {
        Self {
            ui,
            text: text.into(),
            color: Color32::GRAY,
            bg_color: Color32::BLACK,
            size: None,
        }
    }

    pub fn bg_color(mut self, color: Color32) -> Self {
        self.bg_color = color;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn size(mut self, size: impl Into<Vec2>) -> Self {
        self.size = Some(size.into());
        self
    }

    pub fn build(self) -> Response {
        Frame::NONE
            .fill(self.bg_color)
            .show(self.ui, |ui| -> Response {
                let label =
                    eframe::egui::Label::new(RichText::new(self.text).color(self.color)).truncate();
                if let Some(size) = self.size {
                    ui.add_sized(size, label)
                } else {
                    ui.add(label)
                }
            })
            .inner
    }
}

// 何だこのひどいコード。テキスト全選択したいだけなのに。
pub fn select_all_text(ui: &Ui, id: Id, text: &str) {
    let mut text_state = TextEditState::load(ui.ctx(), id).unwrap_or_default();
    text_state.cursor.set_char_range(Some(CCursorRange::two(
        CCursor::new(0),
        CCursor::new(text.len()),
    )));
    text_state.store(ui.ctx(), id);
}
