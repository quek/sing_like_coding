use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Note {
    pub delay: u8,
    pub channel: i16,
    pub key: i16,
    pub velocity: f64,
    #[serde(default)]
    pub off: bool,
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

pub fn midi_to_note_name(midi: i16) -> Option<String> {
    if midi > 127 {
        return None;
    }
    let note_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let note = note_names[(midi % 12) as usize];
    let octave = (midi / 12).wrapping_sub(1); // C4 = 60
    Some(format!("{}{}", note, octave))
}

pub fn note_name_to_midi(note: &str) -> Option<i16> {
    if note.len() < 2 {
        return None;
    }

    let note_map = [
        ("C", 0),
        ("C#", 1),
        ("D", 2),
        ("D#", 3),
        ("E", 4),
        ("F", 5),
        ("F#", 6),
        ("G", 7),
        ("G#", 8),
        ("A", 9),
        ("A#", 10),
        ("B", 11),
    ];

    let note = note.to_uppercase();
    let (note_str, octave_str) = note.trim().split_at(note.len() - 1);
    let (base, octave_offset) = if let Ok(octave) = octave_str.parse::<i16>() {
        (note_str, octave)
    } else {
        let (note_str, rest) = note.split_at(note.len() - 2);
        if let Ok(octave) = rest.parse::<i16>() {
            (note_str, octave)
        } else {
            return None;
        }
    };

    let semitone = note_map.iter().find(|(n, _)| *n == base)?.1;
    let midi = (octave_offset + 1) * 12 + semitone;
    if midi >= 0 && midi <= 127 {
        Some(midi)
    } else {
        None
    }
}
