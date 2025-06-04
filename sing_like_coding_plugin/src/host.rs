use std::{path::Path, pin::Pin, sync::mpsc::Sender};

use common::plugin::description::Description;
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

use crate::{plugin::Plugin, plugin_ptr::PluginPtr};

pub struct Host {
    pipe: NamedPipeClient,
    plugin: Pin<Box<Plugin>>,
}

impl Host {
    pub fn new(
        description: &Description,
        pipe_name: String,
        sender: Sender<PluginPtr>,
    ) -> anyhow::Result<Self> {
        let pipe = ClientOptions::new().open(pipe_name)?;

        let mut plugin = Plugin::new(sender);
        plugin.load(Path::new(&description.path), description.index);
        plugin.start()?;
        plugin.gui_open()?;

        Ok(Self { pipe, plugin })
    }
}
