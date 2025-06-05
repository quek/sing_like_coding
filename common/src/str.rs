use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

pub fn to_pcwstr(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

#[macro_export]
macro_rules! cstr {
    ($str:literal) => {
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes()) }
    };
}
