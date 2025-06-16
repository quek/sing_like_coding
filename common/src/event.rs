use clap_sys::id::clap_id;

#[derive(Clone)]
pub enum Event {
    NoteOn(i16, f64, usize),
    NoteOff(i16, usize),
    NoteAllOff,
    ParamValue(clap_id, f64, usize),
}
