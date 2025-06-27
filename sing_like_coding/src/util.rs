use std::sync::atomic::{AtomicUsize, Ordering};

use common::module::ModuleId;
use eframe::egui::{TextStyle, Ui};

pub fn font_mono(ui: &mut Ui) {
    ui.style_mut().override_font_id = Some(TextStyle::Monospace.resolve(ui.style()));
}

pub fn font_reset(ui: &mut Ui) {
    ui.style_mut().override_font_id = None;
}

pub fn with_font_mono<F: FnOnce(&mut Ui)>(ui: &mut Ui, f: F) {
    font_mono(ui);
    f(ui);
    font_reset(ui);
}

#[allow(dead_code)]
pub fn with_font_mono_result<F, R>(ui: &mut Ui, f: F) -> R
where
    F: FnOnce(&mut Ui) -> R,
{
    font_mono(ui);
    let result = f(ui);
    font_reset(ui);
    result
}

pub fn midi_tick_to_line_delay(tick: u32, ticks_per_line: u32) -> (usize, u8) {
    let line = (tick / ticks_per_line) as usize;
    let tick_in_line = tick % ticks_per_line;
    let delay = ((tick_in_line * 256) / ticks_per_line).min(255) as u8;
    (line, delay)
}

pub fn is_subsequence_case_insensitive(name: &str, query: &str) -> bool {
    let mut query_chars = query.chars().map(|c| c.to_ascii_lowercase());
    let mut current_q = query_chars.next();

    for c in name.chars() {
        if let Some(qc) = current_q {
            if qc == c.to_ascii_lowercase() {
                current_q = query_chars.next();
            }
        } else {
            break;
        }
    }
    current_q.is_none()
}

static GLOBAL_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub fn next_id() -> ModuleId {
    GLOBAL_COUNTER.fetch_add(1, Ordering::Relaxed)
}
