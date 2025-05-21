use std::{
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
    pin::Pin,
    ptr::null_mut,
};

use anyhow::Result;
use clap_sys::{
    entry::clap_plugin_entry,
    ext::gui::{
        clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI, CLAP_WINDOW_API_WIN32,
    },
    factory::plugin_factory::clap_plugin_factory,
    host::clap_host,
    plugin::clap_plugin,
    version::CLAP_VERSION,
};
use libloading::{Library, Symbol};
use window::create_handler;

mod window;

pub struct Host {
    _name: Pin<CString>,
    _vender: Pin<CString>,
    _url: Pin<CString>,
    _version: Pin<CString>,
    clap_host: clap_host,
    lib: Option<Library>,
    plugin: Option<clap_plugin>,
}

impl Host {
    fn new() -> Self {
        let name: Pin<CString> = Pin::new(CString::new("sawavi").unwrap());
        let vender = Pin::new(CString::new("sawavi").unwrap());
        let url = Pin::new(CString::new("https://example.com").unwrap());
        let version = Pin::new(CString::new("0.0.1").unwrap());

        let mut clap_host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: null_mut::<c_void>(),
            name: name.as_ptr(),
            vendor: vender.as_ptr(),
            url: url.as_ptr(),
            version: version.as_ptr(),
            get_extension: Some(Self::get_extension),
            request_restart: None,
            request_process: None,
            request_callback: None,
        };

        clap_host.host_data = &mut clap_host as *mut _ as *mut c_void;

        Self {
            _name: name,
            _vender: vender,
            _url: url,
            _version: version,
            clap_host,
            lib: None,
            plugin: None,
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
                let success = init_fn(CStr::from_bytes_with_nul_unchecked(b".\0").as_ptr());
                if !success {
                    panic!("CLAP init failed");
                }
            } else {
                panic!("CLAP init function is missing");
            }

            let get_factory = entry.get_factory.expect("get_factory function is missing");
            let factory_ptr =
                get_factory(CStr::from_bytes_with_nul_unchecked(b"clap.plugin-factory\0").as_ptr())
                    as *const clap_plugin_factory;
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

    fn edit(&self) -> Result<()> {
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
            if !gui.is_api_supported.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), true) {
                panic!("GUI API not supported");
            }

            // ウィンドウハンドルは OS によって異なる。ここでは仮の値。
            let window_handler = create_handler();
            let parent_window = clap_window_handle {
                win32: window_handler,
            };

            let is_floating = false;
            if gui.create.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), is_floating) == false {
                panic!("GUI create failed");
            }

            if (gui.set_parent.unwrap())(
                plugin,
                &clap_window {
                    api: CLAP_WINDOW_API_WIN32.as_ptr(),
                    specific: parent_window,
                },
            ) == false
            {
                panic!("GUI set_parent failed");
            }

            if (gui.show.unwrap())(plugin) == false {
                panic!("GUI show failed");
            }
        }
        Ok(())
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        if let Some(plugin) = self.plugin {
            unsafe { plugin.destroy.unwrap()(&plugin) };
        }
    }
}

pub fn foo() -> Host {
    let mut host = Host::new();
    let path = Path::new("c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap");
    host.load(path);
    let _ = host.edit();
    host
}
