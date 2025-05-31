use std::{
    ffi::{CStr, CString, OsStr},
    fs,
    path::Path,
};

use anyhow::Result;
use clap_sys::{entry::clap_plugin_entry, factory::plugin_factory::clap_plugin_factory};
use libloading::{Library, Symbol};
use serde::{Deserialize, Serialize};

pub struct ClapManager {
    pub descriptions: Vec<Description>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Description {
    pub id: String,
    pub path: String,
    pub index: usize,
    pub name: String,
}

impl ClapManager {
    pub fn new() -> Self {
        Self {
            descriptions: vec![],
        }
    }

    pub fn scan(&mut self) {
        let plugin_dirs = vec!["C:\\Program Files\\Common Files\\CLAP"];
        for dir in plugin_dirs {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension() == Some(OsStr::new("clap")) || path.is_dir() {
                        let _ = self.scan_plugin_file(&path);
                    }
                }
            }
        }
    }

    fn scan_plugin_file(&mut self, path: &Path) -> Result<()> {
        unsafe {
            if let Ok(lib) = Library::new(path) {
                let entry: Symbol<*const clap_plugin_entry> = lib.get(b"clap_entry\0")?;
                let entry = &**entry;
                let c_path = CString::new(path.to_string_lossy().as_bytes()).unwrap();
                entry.init.unwrap()(c_path.as_ptr());
                let factory =
                    (entry.get_factory.unwrap())(b"clap.plugin-factory\0".as_ptr() as *const _)
                        as *const clap_plugin_factory;

                if factory.is_null() {
                    return Ok(());
                }
                let factory = &*factory;
                let count = (factory.get_plugin_count.unwrap())(factory);
                for index in 0..count {
                    let descriptor = (factory.get_plugin_descriptor.unwrap())(factory, index);
                    let descriptor = &*descriptor;
                    self.descriptions.push(Description {
                        id: CStr::from_ptr(descriptor.id).to_string_lossy().to_string(),
                        path: path.to_str().unwrap().to_string(),
                        index: index as usize,
                        name: CStr::from_ptr(descriptor.name)
                            .to_string_lossy()
                            .to_string(),
                    });
                }

                (entry.deinit.unwrap())();
            }
        }
        Ok(())
    }
}
