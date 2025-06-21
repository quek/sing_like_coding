use std::sync::{mpsc::Sender, Arc, Mutex};

use anyhow::{anyhow, Result};
use common::event::Event;
use midir::{MidiInput, MidiInputConnection};
use wmidi::MidiMessage;

pub struct MidiDevice {
    _connection: MidiInputConnection<()>,
}

impl MidiDevice {
    pub fn list() -> Vec<String> {
        let input = MidiInput::new("SLC").unwrap();
        input
            .ports()
            .iter()
            .filter_map(|port| input.port_name(port).ok())
            .collect()
    }

    pub fn new(
        name: &str,
        sender_midi: Sender<(usize, Event)>,
        track_index: Arc<Mutex<usize>>,
    ) -> Result<Self> {
        let input = MidiInput::new("SLC")?;
        let port = input
            .ports()
            .into_iter()
            .find(|port| input.port_name(port).ok().as_deref() == Some(name))
            .ok_or_else(|| anyhow!("{name} is not found!"))?;
        let connection = input.connect(
            &port,
            "SLC",
            move |_timestamp, data, ()| {
                let Ok(message) = MidiMessage::try_from(data) else {
                    return;
                };
                let event = match message {
                    MidiMessage::NoteOn(_channel, key, velocity) => {
                        Event::NoteOn(key as i16, u8::from(velocity) as f64, 0)
                    }
                    MidiMessage::NoteOff(_channel, key, _velocity) => Event::NoteOff(key as i16, 0),
                    _ => return,
                };
                let _ = sender_midi.send((*track_index.lock().unwrap(), event));
            },
            (),
        )?;
        Ok(Self {
            _connection: connection,
        })
    }
}
