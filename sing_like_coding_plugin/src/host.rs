use std::{path::Path, pin::Pin, sync::mpsc::Sender};

use common::{
    plugin::description::Description,
    process_track_context::ProcessTrackContext,
    protocol::{receive, send, AudioToPlugin, PluginToAudio},
};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

use crate::{plugin::Plugin, plugin_ptr::PluginPtr};

pub struct Host {
    plugin: Pin<Box<Plugin>>,
}

impl Host {
    pub fn new(
        description: &Description,
        pipe_name: String,
        sender: Sender<PluginPtr>,
    ) -> anyhow::Result<Self> {
        let mut plugin = Plugin::new(sender);
        plugin.load(Path::new(&description.path), description.index);
        plugin.start()?;
        plugin.gui_open()?;

        let plugin_ptr: PluginPtr = (&mut plugin).into();
        tokio::spawn(async move {
            process_loop(pipe_name, plugin_ptr).await.unwrap();
        });

        Ok(Self { plugin })
    }
}

async fn process_loop(pipe_name: String, plugin_ptr: PluginPtr) -> anyhow::Result<()> {
    let mut pipe = ClientOptions::new().open(pipe_name)?;
    loop {
        let message: AudioToPlugin = receive(&mut pipe).await?;
        // log::debug!("$$$$ Plugin Audio Thread received {:?}", message);
        match message {
            AudioToPlugin::Process(audio_buffer) => {
                let plugin = unsafe { plugin_ptr.as_mut() };
                let mut context = ProcessTrackContext::default();
                context.nchannels = audio_buffer.buffer.len();
                context.nframes = audio_buffer.buffer[0].len();
                context.play_p = false;
                context.bpm = 120.0;
                context.buffer = audio_buffer;
                plugin.process(&mut context)?;

                send(&mut pipe, &PluginToAudio::Process(context.buffer)).await?;
            }
            AudioToPlugin::B => (),
        }
    }
}
