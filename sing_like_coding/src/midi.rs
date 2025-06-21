use std::sync::mpsc::{channel, Receiver};

use anyhow::{anyhow, Result};
use common::event::Event;
use midir::{Ignore, MidiInput, MidiInputConnection};
use wmidi::MidiMessage;

pub struct MidiDevice {
    _connection: MidiInputConnection<()>,
    receiver_from_callback: Receiver<Event>,
}

impl MidiDevice {
    pub fn new(name: &str) -> Result<Self> {
        let input = MidiInput::new("SLC")?;
        let port = input
            .ports()
            .into_iter()
            .find(|port| input.port_name(port).ok().as_deref() == Some(name))
            .ok_or_else(|| anyhow!("{name} is not found!"))?;
        let (sender_from_callbak, receiver_from_callback) = channel();
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
                let _ = sender_from_callbak.send(event);
            },
            (),
        )?;
        Ok(Self {
            _connection: connection,
            receiver_from_callback,
        })
    }

    pub fn list() -> Result<Vec<String>> {
        let mut input = MidiInput::new("SLC")?;
        input.ignore(Ignore::Sysex | Ignore::Time | Ignore::ActiveSense);
        let ports = input
            .ports()
            .iter()
            .filter_map(|port| input.port_name(port).ok())
            .collect();
        Ok(ports)
    }
}

impl Iterator for MidiDevice {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver_from_callback.try_recv().ok()
    }
}
