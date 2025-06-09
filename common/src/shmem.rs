use std::ffi::CString;

use shared_memory::{Shmem, ShmemConf, ShmemError};
use windows::core::PCSTR;

use crate::str::to_pcstr;

pub const SONG_STATE_NAME: &str = "SingLikeCoding.Song.State";

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

pub fn create_shared_memory<T>(name: &str) -> anyhow::Result<Shmem> {
    let shmem = ShmemConf::new().size(size_of::<T>()).os_id(name).create();
    let shmem = match shmem {
        Ok(s) => s,
        Err(ShmemError::MappingIdExists) => open_shared_memory::<T>(name)?,
        Err(e) => panic!("Unexpected shared memory error: {:?}", e),
    };
    Ok(shmem)
}

pub fn open_shared_memory<T>(name: &str) -> anyhow::Result<Shmem> {
    Ok(ShmemConf::new().size(size_of::<T>()).os_id(name).open()?)
}
