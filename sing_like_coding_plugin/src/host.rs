use std::{
    ffi::CString, os::windows::raw::HANDLE, path::Path, pin::Pin, sync::mpsc::Sender,
    time::Duration,
};

use common::{
    plugin::description::Description,
    process_data::ProcessData,
    shmem::{event_request_name, event_response_name, process_data_name},
};
use shared_memory::ShmemConf;
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::ERROR_FILE_NOT_FOUND,
        Storage::FileSystem::SYNCHRONIZE,
        System::Threading::{
            OpenEventA, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE, INFINITE,
            SYNCHRONIZATION_ACCESS_RIGHTS,
        },
    },
};

use crate::{plugin::Plugin, plugin_ptr::PluginPtr};

pub struct Host {
    _plugin: Pin<Box<Plugin>>,
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

        Ok(Self { _plugin: plugin })
    }
}

async fn process_loop(id: usize, plugin_ptr: PluginPtr) -> anyhow::Result<()> {
    let shmem = ShmemConf::new()
        .size(size_of::<ProcessData>())
        .os_id(process_data_name(id))
        .open()?;
    let process_data: &mut ProcessData = unsafe { &mut *(shmem.as_ptr() as *mut ProcessData) };

    let event_request = unsafe {
        OpenEventA(
            EVENT_MODIFY_STATE | SYNCHRONIZATION_ACCESS_RIGHTS(SYNCHRONIZE.0),
            false,
            PCSTR(dbg!(event_request_name(id)).as_ptr().cast()),
        )?
    };
    let event_response = unsafe {
        OpenEventA(
            EVENT_MODIFY_STATE,
            false,
            PCSTR(dbg!(event_response_name(id)).as_ptr().cast()),
        )?
    };

    let plugin = unsafe { plugin_ptr.as_mut() };
    loop {
        unsafe { WaitForSingleObject(event_request, INFINITE) };
        plugin.process(process_data)?;
        unsafe { SetEvent(event_response) }?;
    }
}
