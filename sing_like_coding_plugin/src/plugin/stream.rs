use std::{ffi::c_void, pin::Pin, ptr::null_mut};

use clap_sys::stream::{clap_istream, clap_ostream};

pub struct IStream {
    base: clap_istream,
    buffer: Vec<u8>,
    position: usize,
}

impl IStream {
    pub fn new(buffer: Vec<u8>) -> Pin<Box<Self>> {
        let mut this = Box::pin(Self {
            base: clap_istream {
                ctx: null_mut(),
                read: Some(Self::read),
            },
            buffer,
            position: 0,
        });
        let ctx = this.as_mut().get_mut() as *mut _ as *mut c_void;
        this.as_mut().base.ctx = ctx;
        this
    }

    extern "C" fn read(stream: *const clap_istream, buffer: *mut c_void, size: u64) -> i64 {
        unsafe {
            let this = &mut *((*stream).ctx as *mut Self);
            let remaining = this.buffer.len().saturating_sub(this.position);
            if remaining == 0 {
                return 0;
            }

            let read_len = std::cmp::min(size as usize, remaining);
            let src = &this.buffer[this.position..this.position + read_len];
            let dst = buffer as *mut u8;
            std::ptr::copy_nonoverlapping(src.as_ptr(), dst, read_len);
            this.position += read_len;
            read_len as i64
        }
    }

    pub fn as_raw(self: &Pin<Box<Self>>) -> &clap_istream {
        &self.base
    }
}

pub struct OStream {
    base: clap_ostream,
    buffer: Vec<u8>,
}

impl OStream {
    pub fn new() -> Pin<Box<Self>> {
        let mut this = Box::pin(Self {
            base: clap_ostream {
                ctx: null_mut(),
                write: Some(Self::write),
            },
            buffer: vec![],
        });
        let ctx = this.as_mut().get_mut() as *mut _ as *mut c_void;
        this.as_mut().base.ctx = ctx;
        this
    }

    extern "C" fn write(stream: *const clap_ostream, buffer: *const c_void, size: u64) -> i64 {
        unsafe {
            let this = &mut *((*stream).ctx as *mut Self);
            let bytes = std::slice::from_raw_parts(buffer as *const u8, size as usize);
            this.buffer.extend_from_slice(bytes);
        }

        size as i64
    }

    pub fn as_raw(self: &Pin<Box<Self>>) -> &clap_ostream {
        &self.base
    }

    pub fn into_inner(self: Pin<Box<Self>>) -> Vec<u8> {
        let this = unsafe { Pin::into_inner_unchecked(self) };
        this.buffer
    }
}
