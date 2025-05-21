use std::{
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
    ptr::null_mut,
    sync::{Arc, Mutex},
};

use anyhow::Result;
use clap_sys::{
    entry::clap_plugin_entry,
    ext::gui::{
        clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI, CLAP_WINDOW_API_WIN32,
    },
    factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID},
    host::clap_host,
    plugin::clap_plugin,
    version::CLAP_VERSION,
};
use libloading::{Library, Symbol};
use window::{create_handler, destroy_handler};

mod window;

pub struct Plugin {
    clap_host: clap_host,
    lib: Option<Library>,
    plugin: Option<clap_plugin>,
    frames_per_buffer: Arc<Mutex<usize>>,
    window_handler: Option<*mut c_void>,
}

macro_rules! cstr {
    ($str:literal) => {
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes()) }
    };
}

pub const NAME: &CStr = cstr!("sawavi");
pub const VENDER: &CStr = cstr!("sawavi");
pub const URL: &CStr = cstr!("https://github.com/quek/sawavi");
pub const VERSION: &CStr = cstr!("0.0.1");

impl Plugin {
    fn new(frames_per_buffer: Arc<Mutex<usize>>) -> Self {
        let mut clap_host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: null_mut::<c_void>(),
            name: NAME.as_ptr(),
            vendor: VENDER.as_ptr(),
            url: URL.as_ptr(),
            version: VERSION.as_ptr(),
            get_extension: Some(Self::get_extension),
            request_restart: None,
            request_process: None,
            request_callback: None,
        };

        clap_host.host_data = &mut clap_host as *mut _ as *mut c_void;

        Self {
            clap_host,
            lib: None,
            plugin: None,
            frames_per_buffer,
            window_handler: None,
        }
    }

    unsafe extern "C" fn get_extension(host: *const clap_host, id: *const c_char) -> *const c_void {
        unsafe {
            if host.is_null() || (*host).host_data.is_null() || id.is_null() {
                return std::ptr::null();
            }

            let _host = &*((*host).host_data as *const Self);
            let _id = CStr::from_ptr(id);

            std::ptr::null()
        }
    }

    fn load(&mut self, path: &Path) {
        unsafe {
            let lib = Library::new(path).expect("Failed to load plugin");
            self.lib = Some(lib);
            let entry: Symbol<*const clap_plugin_entry> = self
                .lib
                .as_ref()
                .unwrap()
                .get(b"clap_entry\0")
                .expect("Missing symbol");
            let entry = &**entry;

            if let Some(init_fn) = entry.init {
                let c_path = CString::new(path.to_string_lossy().as_bytes()).unwrap();
                let success = init_fn(c_path.as_ptr());
                if !success {
                    panic!("CLAP init failed");
                }
            } else {
                panic!("CLAP init function is missing");
            }

            let get_factory = entry.get_factory.expect("get_factory function is missing");
            let factory_ptr =
                get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()) as *const clap_plugin_factory;
            if factory_ptr.is_null() {
                panic!("No plugin factory found");
            }
            let factory = &*factory_ptr;

            // plugin ID を取得（index 0 のみ取得例）
            let descriptor = factory.get_plugin_descriptor.unwrap()(factory, 0);
            if descriptor.is_null() {
                panic!("No plugin descriptor");
            }

            let descriptor = &*descriptor;
            let plugin_id = CStr::from_ptr(descriptor.id).to_str().unwrap();
            println!("Found plugin: {}", plugin_id);

            let plugin = factory.create_plugin.unwrap()(factory, &self.clap_host, descriptor.id);
            if plugin.is_null() {
                panic!("Plugin instantiation failed");
            }
            let plugin = &*plugin;
            println!("Found plugin: {:?}", CStr::from_ptr((*plugin.desc).name));

            if !plugin.init.unwrap()(plugin) {
                panic!("Plugin init failed");
            }

            self.plugin = Some(*plugin);
        }
    }

    pub fn gui_open(&mut self) -> Result<()> {
        if self.plugin.is_none() {
            return Ok(());
        }
        let plugin = self.plugin.as_ref().unwrap();
        unsafe {
            // GUI 拡張を取得
            let gui_ptr = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_GUI.as_ptr())
                as *const clap_plugin_gui;

            if gui_ptr.is_null() {
                panic!("Plugin has no GUI extension");
            }

            let gui = &*gui_ptr;

            // GUI を作成
            if !gui.is_api_supported.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false) {
                panic!("GUI API not supported");
            }

            // ウィンドウハンドルは OS によって異なる。ここでは仮の値。
            let window_handler = create_handler();
            self.window_handler = Some(window_handler.clone());
            let parent_window = clap_window_handle {
                win32: window_handler,
            };

            let is_floating = false;
            if gui.create.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), is_floating) == false {
                panic!("GUI create failed");
            }

            if !gui.set_parent.unwrap()(
                plugin,
                &clap_window {
                    api: CLAP_WINDOW_API_WIN32.as_ptr(),
                    specific: parent_window,
                },
            ) {
                panic!("GUI set_parent failed");
            }

            if !gui.show.unwrap()(plugin) {
                panic!("GUI show failed");
            }
        }
        Ok(())
    }

    pub fn gui_close(&mut self) -> Result<()> {
        if self.plugin.is_none() {
            return Ok(());
        }
        let plugin = self.plugin.as_ref().unwrap();
        unsafe {
            let gui = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_GUI.as_ptr())
                as *const clap_plugin_gui;
            let gui = &*gui;
            gui.hide.map(|hide| hide(plugin));
            gui.destroy.map(|destroy| destroy(plugin));
            destroy_handler(self.window_handler.take().unwrap());
        }
        Ok(())
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        if let Some(plugin) = self.plugin {
            unsafe { plugin.destroy.unwrap()(&plugin) };
        }
    }
}

pub fn foo(frames_per_buffer: Arc<Mutex<usize>>) -> Plugin {
    let mut plugin = Plugin::new(frames_per_buffer);
    let path = Path::new("c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap");
    plugin.load(path);
    let _ = plugin.gui_open();
    plugin
}
