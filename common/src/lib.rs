use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

pub mod plugin;
pub mod protocol;

pub const PIPE_NAME: &'static str = r"\\.\pipe\sing_like_coding";
pub const PIPE_BUFFER_SIZE: u32 = 8092;

pub fn to_pcwstr(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}
