#[derive(Clone)]
pub enum Event {
    NoteOn(i16, f64, u8),
    NoteOff(i16, u8),
    NoteAllOff,
}
