use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

pub mod audio_buffer;
pub mod event;
pub mod module;
pub mod plugin;
pub mod process_track_context;
pub mod protocol;

pub const PIPE_NAME: &'static str = r"\\.\pipe\sing_like_coding";
pub const PIPE_BUFFER_SIZE: u32 = 8092;

pub fn to_pcwstr(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

#[macro_export]
macro_rules! cstr {
    ($str:literal) => {
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes()) }
    };
}
