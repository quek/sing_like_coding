use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Note {
    pub key: i16,
    pub velocity: f64,
    pub delay: u8,
    pub off: bool,
    pub channel: i16,
}

impl Note {
    pub fn note_name(&self) -> String {
        if self.off {
            "OFF".to_string()
        } else {
            midi_to_note_name(self.key).unwrap()
        }
    }

    #[allow(dead_code)]
    pub fn set_note_name(&mut self, note_name: &str) {
        note_name_to_midi(note_name).map(|key| self.key = key);
    }
}

impl Default for Note {
    fn default() -> Self {
        Self {
            key: 60,
            velocity: 100.0,
            delay: 0,
            off: false,
            channel: 0,
        }
    }
}

const NOTE_NAMES: &[&str] = &[
    "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
];

pub fn midi_to_note_name(midi: i16) -> Option<String> {
    if midi > 127 {
        return None;
    }
    let note = NOTE_NAMES[(midi % 12) as usize];
    let octave = midi / 12 - 2; // C3 = 60
    Some(format!("{}{:x}", note, (octave & 0x0f)))
}

pub fn note_name_to_midi(note: &str) -> Option<i16> {
    if note.len() != 3 {
        return None;
    }

    let note = note.to_uppercase();
    let (note_str, octave_str) = note.split_at(2);
    let (base, octave_offset) = if let Ok(octave) = i16::from_str_radix(octave_str, 16) {
        (note_str, octave)
    } else {
        return None;
    };

    if let Some(semitone) = NOTE_NAMES.iter().position(|x| *x == base) {
        let midi = (octave_offset + 2) * 12 + semitone as i16;
        if midi >= 0 && midi <= 127 {
            Some(midi)
        } else {
            None
        }
    } else {
        None
    }
}
