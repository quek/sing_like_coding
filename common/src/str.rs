use std::{
    ffi::{CString, OsStr},
    os::windows::ffi::OsStrExt,
};

use windows::core::PCSTR;

pub fn to_pcwstr(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub fn to_pcstr(s: &str) -> anyhow::Result<(PCSTR, CString)> {
    let c_string = CString::new(s)?; // null バイトが含まれていればエラー
    let pcstr = PCSTR(c_string.as_ptr().cast());
    Ok((pcstr, c_string)) // 注意: CString の寿命を保たないと無効ポインタになる
}

#[macro_export]
macro_rules! cstr {
    ($str:literal) => {
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes()) }
    };
}
