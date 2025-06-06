use std::{ffi::c_void, pin::Pin};

use crate::plugin::Plugin;

#[derive(Debug, Clone)]
pub struct PluginPtr(pub *mut c_void);
unsafe impl Send for PluginPtr {}
unsafe impl Sync for PluginPtr {}

impl PluginPtr {
    pub unsafe fn as_mut(&self) -> &mut Plugin {
        unsafe { &mut *(self.0 as *mut Plugin) }
    }
}

impl From<&mut Pin<Box<Plugin>>> for PluginPtr {
    fn from(value: &mut Pin<Box<Plugin>>) -> Self {
        Self(value.as_mut().get_mut() as *mut _ as *mut c_void)
    }
}
