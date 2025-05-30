use std::{ffi::c_void, ops::Range, pin::Pin};

use crate::{
    audio_buffer::AudioBuffer,
    model::{Song, Track},
    plugin::Plugin,
    process_context::Event,
};

#[derive(Debug)]
pub struct PluginPtr(pub *mut c_void);
unsafe impl Send for PluginPtr {}
unsafe impl Sync for PluginPtr {}

pub struct ProcessTrackContext<'a> {
    pub song: &'a Song,
    pub track: &'a Track,
    #[allow(dead_code)]
    pub nchannels: usize,
    pub nframes: usize,
    pub buffer: AudioBuffer,
    pub steady_time: i64,
    pub play_position: &'a Range<i64>,
    pub on_key: &'a mut Option<i16>,
    pub event_list_input: &'a mut Vec<Event>,
    pub plugins: Vec<PluginPtr>,
}

impl<'a> ProcessTrackContext<'a> {
    pub fn new(
        song: &'a Song,
        nchannels: usize,
        nframes: usize,
        steady_time: i64,
        track: &'a Track,
        on_key: &'a mut Option<i16>,
        event_list_input: &'a mut Vec<Event>,
        plugins: &'a mut Vec<Pin<Box<Plugin>>>,
    ) -> Self {
        let mut buffer = AudioBuffer::new();
        buffer.ensure_buffer(nchannels, nframes);
        Self {
            song,
            track,
            nchannels,
            nframes,
            buffer,
            steady_time,
            play_position: &song.play_position,
            on_key,
            event_list_input,
            plugins: plugins
                .iter_mut()
                .map(|x| PluginPtr(x.as_mut().get_mut() as *mut _ as *mut c_void))
                .collect::<Vec<_>>(),
        }
    }
}
