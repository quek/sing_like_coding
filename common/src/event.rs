#[derive(Clone)]
pub enum Event {
    NoteOn(i16, f64, usize),
    NoteOff(i16, usize),
    NoteAllOff,
}
