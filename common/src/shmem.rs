use std::ffi::CString;

use shared_memory::{Shmem, ShmemConf};
use windows::core::PCSTR;

use crate::{audio_buffer::AudioBuffer, str::to_pcstr};

pub fn process_data_name(id: usize) -> String {
    format!("SingLikeCoding.Process.Data.{}", id)
}

pub fn event_request_name(id: usize) -> (PCSTR, CString) {
    to_pcstr(&format!("SingLikeCoding.Process.Request.{}", id)).unwrap()
}

pub fn event_response_name(id: usize) -> (PCSTR, CString) {
    to_pcstr(&format!("SingLikeCoding.Process.Response.{}", id)).unwrap()
}

pub fn event_quit_name(id: usize) -> (PCSTR, CString) {
    to_pcstr(&format!("SingLikeCoding.Process.Quit.{}", id)).unwrap()
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
