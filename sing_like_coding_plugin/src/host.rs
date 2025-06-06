use std::{path::Path, pin::Pin, sync::mpsc::Sender};

use common::{
    plugin::description::Description,
    process_data::ProcessData,
    shmem::{event_request_name, event_response_name, process_data_name},
};
use shared_memory::ShmemConf;
use windows::Win32::{
    Storage::FileSystem::SYNCHRONIZE,
    System::Threading::{
        OpenEventA, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE,
        SYNCHRONIZATION_ACCESS_RIGHTS,
    },
};

use crate::{plugin::Plugin, plugin_ptr::PluginPtr};

pub struct Host {
    pub plugin: Pin<Box<Plugin>>,
}

impl Host {
    pub fn new(
        id: usize,
        description: &Description,
        sender: Sender<PluginPtr>,
    ) -> anyhow::Result<Self> {
        let mut plugin = Plugin::new(sender);
        plugin.load(Path::new(&description.path), description.index);
        plugin.start()?;
        plugin.gui_open()?;

        let plugin_ptr: PluginPtr = (&mut plugin).into();
        tokio::spawn(async move {
            process_loop(id, plugin_ptr).await.unwrap();
        });

        Ok(Self { plugin })
    }
}

async fn process_loop(id: usize, plugin_ptr: PluginPtr) -> anyhow::Result<()> {
    let shmem = ShmemConf::new()
        .size(size_of::<ProcessData>())
        .os_id(process_data_name(id))
        .open()?;
    let process_data: &mut ProcessData = unsafe { &mut *(shmem.as_ptr() as *mut ProcessData) };

    let (event_name, _x) = event_request_name(id);
    let event_request = unsafe {
        OpenEventA(
            EVENT_MODIFY_STATE | SYNCHRONIZATION_ACCESS_RIGHTS(SYNCHRONIZE.0),
            false,
            event_name,
        )?
    };

    let (event_name, _x) = event_response_name(id);
    let event_response = unsafe { OpenEventA(EVENT_MODIFY_STATE, false, event_name)? };

    let plugin = unsafe { plugin_ptr.as_mut() };
    loop {
        // log::debug!("$$$$ host will WaitForSingleObject process request");
        unsafe { WaitForSingleObject(event_request, INFINITE) };
        // log::debug!("$$$$ before plugin.process");
        plugin.process(process_data)?;
        unsafe { SetEvent(event_response) }?;
    }
}
