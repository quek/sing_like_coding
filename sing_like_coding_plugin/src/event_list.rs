use std::{ffi::c_void, pin::Pin, ptr::null_mut};

use clap_sys::{
    events::{
        clap_event_header, clap_event_midi, clap_event_note, clap_event_param_value,
        clap_input_events, clap_output_events, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_MIDI,
        CLAP_EVENT_MIDI2, CLAP_EVENT_MIDI_SYSEX, CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_END,
        CLAP_EVENT_NOTE_EXPRESSION, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
        CLAP_EVENT_PARAM_GESTURE_BEGIN, CLAP_EVENT_PARAM_GESTURE_END, CLAP_EVENT_PARAM_MOD,
        CLAP_EVENT_PARAM_VALUE, CLAP_EVENT_TRANSPORT,
    },
    id::clap_id,
};
use common::event::Event;

pub struct EventListInput {
    events: Vec<*const clap_event_header>,
    clap_input_events: clap_input_events,
}

impl EventListInput {
    pub fn new() -> Pin<Box<Self>> {
        let mut this = Box::pin(Self {
            events: vec![],
            clap_input_events: clap_input_events {
                ctx: null_mut(),
                size: Some(Self::size),
                get: Some(Self::get),
            },
        });
        let ptr = this.as_mut().get_mut() as *mut _ as *mut c_void;
        this.as_mut().clap_input_events.ctx = ptr;
        this
    }

    pub fn as_clap_input_events(&self) -> &clap_input_events {
        &self.clap_input_events
    }

    extern "C" fn size(list: *const clap_input_events) -> u32 {
        let this = unsafe { &*((*list).ctx as *const Self) };
        //log::debug!("EventList size {}", this.events.len() as u32);
        this.events.len() as u32
    }

    extern "C" fn get(list: *const clap_input_events, index: u32) -> *const clap_event_header {
        // log::debug!("EventList get {index}");
        let this = unsafe { &*((*list).ctx as *const Self) };
        this.events
            .get(index as usize)
            .copied()
            .unwrap_or(std::ptr::null())
    }

    pub fn note_on(&mut self, key: i16, channel: i16, velocity: f64, time: u32) {
        let event = Box::new(clap_event_note {
            header: clap_event_header {
                size: size_of::<clap_event_note>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_NOTE_ON,
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel,
            key,
            velocity,
        });
        self.events
            .push(Box::into_raw(event) as *const clap_event_header);
    }

    pub fn note_off(&mut self, key: i16, channel: i16, velocity: f64, time: u32) {
        let event = Box::new(clap_event_note {
            header: clap_event_header {
                size: size_of::<clap_event_note>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_NOTE_OFF,
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel,
            key,
            velocity,
        });
        self.events
            .push(Box::into_raw(event) as *const clap_event_header);
    }

    pub fn param_value(&mut self, param_id: clap_id, value: f64, time: u32) {
        let event = Box::new(clap_event_param_value {
            header: clap_event_header {
                size: size_of::<clap_event_note>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_PARAM_VALUE,
                flags: 0,
            },
            param_id,
            cookie: null_mut(),
            note_id: -1,
            port_index: 0,
            channel: 0,
            key: 0,
            value,
        });
        self.events
            .push(Box::into_raw(event) as *const clap_event_header);
    }

    pub fn clear(&mut self) {
        for &ptr in &self.events {
            if !ptr.is_null() {
                unsafe {
                    match (*ptr).type_ {
                        CLAP_EVENT_NOTE_ON
                        | CLAP_EVENT_NOTE_OFF
                        | CLAP_EVENT_NOTE_CHOKE
                        | CLAP_EVENT_NOTE_END => {
                            drop(Box::from_raw(ptr as *mut clap_event_note));
                        }
                        CLAP_EVENT_MIDI => {
                            drop(Box::from_raw(ptr as *mut clap_event_midi));
                        }
                        CLAP_EVENT_PARAM_VALUE => {
                            drop(Box::from_raw(ptr as *mut clap_event_param_value));
                        }
                        _ => {
                            unreachable!();
                        }
                    }
                }
            }
        }
        self.events.clear();
    }
}

impl Drop for EventListInput {
    fn drop(&mut self) {
        self.clear();
    }
}

pub struct EventListOutput {
    pub events: Vec<Event>,
    clap_output_events: clap_output_events,
    pub samples_per_delay: f64,
}

impl EventListOutput {
    pub fn new() -> Pin<Box<Self>> {
        let mut this = Box::pin(Self {
            events: vec![],
            clap_output_events: clap_output_events {
                ctx: null_mut(),
                try_push: Some(Self::try_push),
            },
            samples_per_delay: 1.0,
        });
        let ptr = this.as_mut().get_mut() as *mut _ as *mut c_void;
        this.as_mut().clap_output_events.ctx = ptr;
        this
    }

    pub fn as_clap_output_events(&self) -> &clap_output_events {
        &self.clap_output_events
    }

    extern "C" fn try_push(
        list: *const clap_output_events,
        event: *const clap_event_header,
    ) -> bool {
        let this = unsafe { &mut *((*list).ctx as *mut Self) };
        let event_header = unsafe { &*event };
        let delay = (event_header.time as f64 / this.samples_per_delay).round() as usize;
        match event_header.type_ {
            CLAP_EVENT_NOTE_ON => {
                let event_note: &clap_event_note = unsafe { &*(event as *const clap_event_note) };
                this.events
                    .push(Event::NoteOn(event_note.key, event_note.velocity, delay))
            }
            CLAP_EVENT_NOTE_OFF => {
                let event_note: &clap_event_note = unsafe { &*(event as *const clap_event_note) };
                this.events.push(Event::NoteOff(event_note.key, delay))
            }
            CLAP_EVENT_NOTE_CHOKE => {}
            CLAP_EVENT_NOTE_END => {}
            CLAP_EVENT_NOTE_EXPRESSION => {}
            CLAP_EVENT_PARAM_VALUE => {
                let event_param_value = unsafe { &*(event as *const clap_event_param_value) };
                this.events.push(Event::ParamValue(
                    0,
                    event_param_value.param_id,
                    event_param_value.value,
                    delay,
                ))
            }
            CLAP_EVENT_PARAM_MOD => {}
            CLAP_EVENT_PARAM_GESTURE_BEGIN => {}
            CLAP_EVENT_PARAM_GESTURE_END => {}
            CLAP_EVENT_TRANSPORT => {}
            CLAP_EVENT_MIDI => {}
            CLAP_EVENT_MIDI_SYSEX => {}
            CLAP_EVENT_MIDI2 => {}
            _ => {
                log::warn!("Unknown event type {}!", event_header.type_);
            }
        }
        true
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}
