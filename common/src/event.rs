use clap_sys::id::clap_id;

#[derive(Clone, Debug)]
pub enum Event {
    NoteOn(i16, f64, usize),
    NoteOff(i16, usize),
    NoteAllOff,
    ParamValue(usize, clap_id, f64, usize),
}
