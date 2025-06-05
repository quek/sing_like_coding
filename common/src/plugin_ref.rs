use windows::Win32::{
    Foundation::HANDLE,
    System::Threading::{CreateEventA, SetEvent, WaitForSingleObject, INFINITE},
};

use crate::{
    process_data::ProcessData,
    shmem::{event_request_name, event_response_name},
};

#[derive(Clone)]
pub struct PluginRef {
    pub ptr: *mut ProcessData,
    pub event_request: HANDLE,
    pub event_response: HANDLE,
}

impl PluginRef {
    pub fn new(id: usize, ptr: *mut ProcessData) -> anyhow::Result<Self> {
        let (event_name, _x) = event_request_name(id);
        let event_request = unsafe {
            CreateEventA(
                None,
                false.into(), // 自動リセット
                false.into(), // 初期非シグナル
                event_name,
            )?
        };

        let (event_name, _x) = event_response_name(id);
        let event_response = unsafe {
            CreateEventA(
                None,
                false.into(), // 自動リセット
                false.into(), // 初期非シグナル
                event_name,
            )?
        };

        Ok(Self {
            ptr,
            event_request,
            event_response,
        })
    }

    pub fn process(&mut self) -> anyhow::Result<()> {
        unsafe { SetEvent(self.event_request) }?;
        unsafe { WaitForSingleObject(self.event_response, INFINITE) };
        Ok(())
    }

    pub fn process_data(&mut self) -> &mut ProcessData {
        let x: &mut ProcessData = unsafe { &mut *(self.ptr) };
        x
    }
}
