use std::ffi::CString;

use shared_memory::{Shmem, ShmemConf};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::WAIT_OBJECT_0,
        Storage::FileSystem::SYNCHRONIZE,
        System::Threading::{
            CreateEventA, OpenEventA, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
            SYNCHRONIZATION_ACCESS_RIGHTS,
        },
    },
};

use crate::audio_buffer::AudioBuffer;

pub fn process_data_name(id: usize) -> String {
    format!("SingLikeCoding.Process.Data.{}", id)
}

pub fn event_request_name(id: usize) -> CString {
    CString::new(format!("SingLikeCoding.Process.Request.{}", id)).unwrap()
}

pub fn event_response_name(id: usize) -> CString {
    CString::new(format!("SingLikeCoding.Process.Response.{}", id)).unwrap()
}

pub const SHMEM_NAME: &str = "MySharedMemory";
pub const EVENT_NAME: &str = "MyPluginEvent";

pub fn create_shared_memory() -> anyhow::Result<Shmem> {
    Ok(ShmemConf::new()
        .size(size_of::<AudioBuffer>())
        .os_id(SHMEM_NAME)
        .create()?)
}

pub fn open_shared_memory() -> anyhow::Result<Shmem> {
    Ok(ShmemConf::new()
        .size(size_of::<AudioBuffer>())
        .os_id(SHMEM_NAME)
        .open()?)
}

pub fn foo() -> anyhow::Result<()> {
    ShmemConf::new()
        .size(size_of::<AudioBuffer>())
        .os_id(SHMEM_NAME)
        .create()?;
    Ok(())
}

pub fn proc_host() -> anyhow::Result<()> {
    let shmem = create_shared_memory()?;
    let buffer: &mut AudioBuffer = unsafe { &mut *(shmem.as_ptr() as *mut AudioBuffer) };

    // Windowsイベント作成
    let event = unsafe {
        CreateEventA(
            None,
            false.into(), // 自動リセット
            false.into(), // 初期非シグナル
            PCSTR(EVENT_NAME.as_ptr()),
        )?
    };
    assert!(!event.is_invalid());

    buffer.zero();

    unsafe { SetEvent(event)? };

    Ok(())
}

pub fn proc_plugin() -> anyhow::Result<()> {
    let shmem = open_shared_memory()?;
    let buffer: &AudioBuffer = unsafe { &*(shmem.as_ptr() as *const AudioBuffer) };

    let event = unsafe {
        OpenEventA(
            EVENT_MODIFY_STATE | SYNCHRONIZATION_ACCESS_RIGHTS(SYNCHRONIZE.0),
            false,
            PCSTR(EVENT_NAME.as_ptr()),
        )?
    };
    assert!(!event.is_invalid());

    loop {
        let status = unsafe { WaitForSingleObject(event, 3000) };

        if status == WAIT_OBJECT_0 {
            println!("Plugin read: {:?}", buffer.constant_mask);
        } else {
            println!("Timeout or error");
            break;
        }
    }

    Ok(())
}
