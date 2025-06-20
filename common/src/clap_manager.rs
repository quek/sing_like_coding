use std::{
    ffi::{c_char, CStr, CString, OsStr},
    fs::{self, create_dir_all, metadata, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use anyhow::Result;
use clap_sys::{entry::clap_plugin_entry, factory::plugin_factory::clap_plugin_factory};
use libloading::{Library, Symbol};

use crate::{plugin::description::Description, util::dir_user_setting};

pub struct ClapManager {
    pub setting_path: PathBuf,
    pub descriptions: Vec<Description>,
}

impl ClapManager {
    pub fn new() -> Self {
        let setting_path = dir_user_setting().join("claps.json");
        let mut this = Self {
            setting_path,
            descriptions: vec![],
        };
        let _ = this.load();
        this
    }

    pub fn description(&self, id: &String) -> Option<&Description> {
        self.descriptions.iter().find(|x| x.id == *id)
    }

    pub fn load(&mut self) -> Result<()> {
        let file = File::open(&self.setting_path)?;
        let reader = BufReader::new(file);
        self.descriptions = serde_json::from_reader(reader)?;
        Ok(())
    }

    pub fn scan(&mut self) {
        self.descriptions.clear();
        for path in self.find_clap_files(&Path::new("C:\\Program Files\\Common Files\\CLAP")) {
            log::debug!("path {path:?}");
            log::debug!("extension {:?}", path.extension());
            if path.extension() == Some(OsStr::new("clap")) || path.is_dir() {
                match self.scan_plugin_file(&path) {
                    Ok(_) => (),
                    Err(error) => log::error!("scan clap file is failed! {:?} {:?}", path, error),
                }
            }
        }
        self.descriptions.sort_by_key(|x| x.name.clone());
        self.save();
    }

    fn features_to_vec(&self, features: *const *const c_char) -> Vec<String> {
        let mut result = Vec::new();
        if features.is_null() {
            return result;
        }

        unsafe {
            let mut ptr = features;
            while !(*ptr).is_null() {
                let c_str = CStr::from_ptr(*ptr);
                result.push(c_str.to_string_lossy().to_string());
                ptr = ptr.add(1);
            }
        }

        result
    }

    fn find_clap_files(&self, dir: &Path) -> Vec<PathBuf> {
        let mut result = Vec::new();
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    // サブディレクトリを再帰的に探索
                    result.extend(self.find_clap_files(&path));
                } else if path.extension().and_then(OsStr::to_str) == Some("clap") {
                    // *.clap ファイルだけ追加
                    result.push(path);
                }
            }
        }
        result
    }

    fn save(&mut self) {
        if let Some(parent) = self.setting_path.parent() {
            create_dir_all(parent).unwrap();
        }
        let mut file = File::create(&self.setting_path).unwrap();
        let json = serde_json::to_string_pretty(&self.descriptions).unwrap();
        file.write_all(json.as_bytes()).unwrap();
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
                        modified: metadata(path)
                            .unwrap()
                            .modified()
                            .unwrap()
                            .elapsed()
                            .unwrap()
                            .as_secs(),
                        index,
                        name: CStr::from_ptr(descriptor.name)
                            .to_string_lossy()
                            .to_string(),
                        vender: CStr::from_ptr(descriptor.vendor)
                            .to_string_lossy()
                            .to_string(),
                        version: CStr::from_ptr(descriptor.version)
                            .to_string_lossy()
                            .to_string(),
                        description: CStr::from_ptr(descriptor.description)
                            .to_string_lossy()
                            .to_string(),
                        features: self.features_to_vec(descriptor.features),
                    });
                }

                (entry.deinit.unwrap())();
            }
        }
        Ok(())
    }
}
