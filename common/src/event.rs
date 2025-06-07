#[derive(Clone)]
pub enum Event {
    NoteOn(i16, f64),
    NoteOff(i16),
    NoteAllOff,
}
