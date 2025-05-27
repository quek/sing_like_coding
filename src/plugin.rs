use std::{
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
    pin::Pin,
    ptr::{null, null_mut},
    sync::mpsc::Sender,
};

use anyhow::Result;
use clap_sys::{
    audio_buffer::clap_audio_buffer,
    entry::clap_plugin_entry,
    events::{
        clap_event_header, clap_event_midi, clap_event_note, clap_input_events, clap_output_events,
        CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_MIDI, CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_END,
        CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
    },
    ext::{
        audio_ports::{clap_host_audio_ports, CLAP_EXT_AUDIO_PORTS},
        gui::{
            clap_host_gui, clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI,
            CLAP_WINDOW_API_WIN32,
        },
        latency::{clap_host_latency, CLAP_EXT_LATENCY},
        log::{
            clap_host_log, clap_log_severity, CLAP_EXT_LOG, CLAP_LOG_DEBUG, CLAP_LOG_ERROR,
            CLAP_LOG_INFO, CLAP_LOG_WARNING,
        },
        params::{
            clap_host_params, clap_param_clear_flags, clap_param_rescan_flags, CLAP_EXT_PARAMS,
        },
    },
    factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID},
    host::clap_host,
    id::clap_id,
    plugin::clap_plugin,
    process::{clap_process, CLAP_PROCESS_ERROR},
    version::{clap_version_is_compatible, CLAP_VERSION},
};
use libloading::{Library, Symbol};
use window::{create_handler, destroy_handler};

mod window;

pub struct Plugin {
    clap_host: clap_host,
    lib: Option<Library>,
    pub plugin: Option<*const clap_plugin>,
    gui: Option<*const clap_plugin_gui>,
    gui_open_p: bool,
    window_handler: Option<*mut c_void>,
    process_start_p: bool,
    callback_request_sender: Sender<*const clap_plugin>,
    host_audio_ports: clap_host_audio_ports,
    host_gui: clap_host_gui,
    host_latency: clap_host_latency,
    host_log: clap_host_log,
    host_params: clap_host_params,
    pub gui_context: Option<eframe::egui::Context>,
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
    pub fn new(callback_request_sender: Sender<*const clap_plugin>) -> Pin<Box<Self>> {
        let clap_host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: null_mut::<c_void>(),
            name: NAME.as_ptr(),
            vendor: VENDER.as_ptr(),
            url: URL.as_ptr(),
            version: VERSION.as_ptr(),
            get_extension: Some(Self::get_extension),
            request_restart: Some(Self::request_restart),
            request_process: Some(Self::request_process),
            request_callback: Some(Self::request_callback),
        };

        let host_audio_ports = clap_host_audio_ports {
            is_rescan_flag_supported: Some(Self::audio_ports_is_rescan_flag_supported),
            rescan: Some(Self::audio_ports_rescan),
        };

        let host_gui = clap_host_gui {
            resize_hints_changed: Some(Self::gui_resize_hints_changed),
            request_resize: Some(Self::gui_request_resize),
            request_show: Some(Self::gui_request_show),
            request_hide: Some(Self::gui_request_hide),
            closed: Some(Self::gui_closed),
        };

        let host_latency = clap_host_latency {
            changed: Some(Self::latency_changed),
        };

        let host_log = clap_host_log {
            log: Some(Self::log_log),
        };

        let host_params = clap_host_params {
            rescan: Some(Self::params_rescan),
            clear: Some(Self::params_clear),
            request_flush: Some(Self::params_request_flush),
        };

        let mut this = Box::pin(Self {
            clap_host,
            lib: None,
            plugin: None,
            gui: None,
            gui_open_p: false,
            window_handler: None,
            process_start_p: false,
            callback_request_sender,
            host_audio_ports,
            host_gui,
            host_latency,
            host_log,
            host_params,
            gui_context: None,
        });

        let ptr = this.as_mut().get_mut() as *mut _ as *mut c_void;
        this.as_mut().clap_host.host_data = ptr;

        this
    }

    unsafe extern "C" fn audio_ports_is_rescan_flag_supported(
        _host: *const clap_host,
        _flag: u32,
    ) -> bool {
        log::debug!("audio_ports_is_rescan_flag_supported");
        false
    }

    unsafe extern "C" fn audio_ports_rescan(_host: *const clap_host, _flag: u32) {
        log::debug!("audio_ports_rescan");
    }

    unsafe extern "C" fn gui_resize_hints_changed(_host: *const clap_host) {
        log::debug!("gui_resize_hints_changed");
    }

    unsafe extern "C" fn gui_request_resize(
        _host: *const clap_host,
        _width: u32,
        _height: u32,
    ) -> bool {
        log::debug!("gui_request_resize");
        true
    }

    unsafe extern "C" fn gui_request_show(_host: *const clap_host) -> bool {
        log::debug!("gui_request_show");
        true
    }

    unsafe extern "C" fn gui_request_hide(_host: *const clap_host) -> bool {
        log::debug!("gui_request_hide");
        true
    }

    unsafe extern "C" fn gui_closed(_host: *const clap_host, _was_destroyed: bool) {
        log::debug!("gui_closed");
    }

    unsafe extern "C" fn latency_changed(_host: *const clap_host) {
        log::debug!("latency_changed");
    }

    unsafe extern "C" fn log_log(
        _host: *const clap_host,
        severity: clap_log_severity,
        msg: *const c_char,
    ) {
        let msg = unsafe { CStr::from_ptr(msg) };

        match severity {
            CLAP_LOG_DEBUG => log::debug!("{:?}", msg),
            CLAP_LOG_INFO => log::info!("{:?}", msg),
            CLAP_LOG_WARNING => log::warn!("{:?}", msg),
            CLAP_LOG_ERROR => log::error!("{:?}", msg),
            _ => log::debug!("severity {severity} {:?}", msg),
        }
    }

    unsafe extern "C" fn params_rescan(_host: *const clap_host, _flags: clap_param_rescan_flags) {
        log::debug!("params_rescan");
    }

    unsafe extern "C" fn params_clear(
        _host: *const clap_host,
        _param_id: clap_id,
        _flags: clap_param_clear_flags,
    ) {
        log::debug!("params_clear");
    }

    unsafe extern "C" fn params_request_flush(_host: *const clap_host) {
        log::debug!("params_request_flush");
    }

    unsafe extern "C" fn request_callback(host: *const clap_host) {
        log::debug!("request_callback");
        let this = unsafe { &mut *((*host).host_data as *mut Self) };
        let plugin = this.plugin.unwrap();
        this.callback_request_sender.send(plugin).unwrap();
        this.gui_context.as_ref().map(|x| x.request_repaint());
    }

    unsafe extern "C" fn request_process(_host: *const clap_host) {
        log::debug!("request_process");
    }

    unsafe extern "C" fn request_restart(host: *const clap_host) {
        log::debug!("request_restart");
        let this = unsafe { &mut *((*host).host_data as *mut Self) };
        this.stop().unwrap();
        this.start().unwrap();
    }

    unsafe extern "C" fn get_extension(host: *const clap_host, id: *const c_char) -> *const c_void {
        unsafe {
            log::debug!("get_extension {:?}", CStr::from_ptr(id).to_str());
            if host.is_null() || (*host).host_data.is_null() || id.is_null() {
                return std::ptr::null();
            }

            let host = &*((*host).host_data as *const Self);
            let id = CStr::from_ptr(id);

            if id == CLAP_EXT_AUDIO_PORTS {
                return &host.host_audio_ports as *const _ as *const c_void;
            }
            if id == CLAP_EXT_GUI {
                return &host.host_gui as *const _ as *const c_void;
            }
            if id == CLAP_EXT_LATENCY {
                return &host.host_latency as *const _ as *const c_void;
            }
            if id == CLAP_EXT_LOG {
                return &host.host_log as *const _ as *const c_void;
            }
            if id == CLAP_EXT_PARAMS {
                return &host.host_params as *const _ as *const c_void;
            }
            std::ptr::null()
        }
    }

    pub fn load(&mut self, path: &Path) {
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
            if !clap_version_is_compatible(descriptor.clap_version) {
                panic!("Incompatible clap version {:?}", descriptor.clap_version);
            }
            log::debug!("descriptor {:?}", descriptor);
            let plugin_id = CStr::from_ptr(descriptor.id).to_str().unwrap();
            println!("Found plugin: {}", plugin_id);

            let clap_plugin =
                factory.create_plugin.unwrap()(factory, &self.clap_host, descriptor.id);
            if clap_plugin.is_null() {
                panic!("Plugin instantiation failed");
            }
            let plugin = &*clap_plugin;
            println!("Found plugin: {:?}", CStr::from_ptr((*plugin.desc).name));

            if !plugin.init.unwrap()(plugin) {
                panic!("Plugin init failed");
            }

            let gui = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_GUI.as_ptr())
                as *const clap_plugin_gui;
            if !gui.is_null() {
                self.gui = Some(gui);
            }

            self.plugin = Some(plugin);
        }
    }

    pub fn gui_available(&self) -> bool {
        if self.gui.is_none() {
            return false;
        }
        let gui = unsafe { &*self.gui.unwrap() };
        gui.is_api_supported.is_some()
            && gui.get_preferred_api.is_some()
            && gui.create.is_some()
            && gui.destroy.is_some()
            && gui.set_scale.is_some()
            && gui.get_size.is_some()
            && gui.can_resize.is_some()
            && gui.get_resize_hints.is_some()
            && gui.adjust_size.is_some()
            && gui.set_size.is_some()
            && gui.set_parent.is_some()
            && gui.set_transient.is_some()
            && gui.suggest_title.is_some()
            && gui.show.is_some()
            && gui.hide.is_some()
    }

    pub fn gui_open(&mut self) -> Result<()> {
        if self.gui_open_p || !self.gui_available() {
            return Ok(());
        }
        let plugin = unsafe { &*(self.plugin.unwrap()) };
        let gui = unsafe { &*self.gui.unwrap() };
        unsafe {
            if !gui.is_api_supported.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false) {
                log::debug!("GUI API not supported");
                return Ok(());
            }

            let is_floating = false;
            if gui.create.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), is_floating) == false {
                panic!("GUI create failed");
            }

            if !gui.set_scale.unwrap()(plugin, 1.0) {
                // If the plugin prefers to work out the scaling
                // factor itself by querying the OS directly, then
                // ignore the call.
                log::debug!("GUI set_scale failed");
            }

            let resizable = gui.can_resize.unwrap()(plugin);
            let mut width = 0;
            let mut height = 0;
            if !gui.get_size.unwrap()(plugin, &mut width, &mut height) {
                panic!("GUI get_size failed");
            }

            let window_handler = create_handler(resizable, width, height);
            self.window_handler = Some(window_handler.clone());
            let parent_window = clap_window_handle {
                win32: window_handler,
            };

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
                // VCV Rack だと false になる
                log::debug!("GUI show failed");
            }

            self.gui_open_p = true;
        }
        Ok(())
    }

    pub fn gui_close(&mut self) -> Result<()> {
        if !self.gui_open_p || !self.gui_available() {
            return Ok(());
        }
        let plugin = unsafe { &*(self.plugin.unwrap()) };
        let gui = unsafe { &*self.gui.unwrap() };
        unsafe {
            gui.hide.unwrap()(plugin);
            gui.destroy.unwrap()(plugin);
            destroy_handler(self.window_handler.take().unwrap());
        }
        self.gui_open_p = false;
        Ok(())
    }

    pub fn process(
        &mut self,
        buffer: &mut Vec<Vec<f32>>,
        frames_count: u32,
        steady_time: i64,
    ) -> Result<()> {
        //log::debug!("plugin.process frames_count {frames_count}");

        let mut in_buf0 = vec![0.0; frames_count as usize];
        let mut in_buf1 = vec![0.0; frames_count as usize];
        let mut in_buffer = vec![in_buf0.as_mut_ptr(), in_buf1.as_mut_ptr()];

        let audio_input = clap_audio_buffer {
            data32: in_buffer.as_mut_ptr(),
            data64: null_mut::<*mut f64>(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut audio_inputs = [audio_input];

        let mut out_buffer = vec![];
        for channel in buffer.iter_mut() {
            out_buffer.push(channel.as_mut_ptr());
        }

        let audio_output = clap_audio_buffer {
            data32: out_buffer.as_mut_ptr(),
            data64: null_mut::<*mut f64>(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut audio_outputs = [audio_output];

        let mut event_list = EventList::new();
        if steady_time == 0 {
            event_list.note_on(60, 0, 100.0, 0);
            event_list.note_on(64, 0, 100.0, 0);
            event_list.note_on(67, 0, 100.0, 0);
        }
        let in_events = event_list.as_clap_input_events();
        let mut event_list_output = EventListOutput::new();
        let out_events = event_list_output.as_clap_output_events();
        let prc = clap_process {
            steady_time,
            frames_count,
            transport: null(),
            audio_inputs: audio_inputs.as_mut_ptr(),
            audio_outputs: audio_outputs.as_mut_ptr(),
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events,
            out_events,
        };
        let plugin = unsafe { &*(self.plugin.unwrap()) };
        // log::debug!("before process");
        let status = unsafe { plugin.process.unwrap()(plugin, &prc) };
        // log::debug!("after process {status}");
        event_list.clear();
        if status == CLAP_PROCESS_ERROR {
            panic!("process returns CLAP_PROCESS_ERROR");
        }

        Ok(())
    }

    pub fn start(&mut self) -> Result<()> {
        if self.process_start_p {
            return Ok(());
        }
        let plugin = unsafe { &*(self.plugin.unwrap()) };
        // let sample_rate = self.supported_stream_config.sample_rate().0 as f64;
        // min_frames_count が 0 だと activate できないみたい
        // let (min_frames_count, max_frames_count): (u32, u32) =
        //     match self.supported_stream_config.buffer_size() {
        //         cpal::SupportedBufferSize::Range { min, max } => (*min, *max),
        //         cpal::SupportedBufferSize::Unknown => (64, 4096),
        //     };
        unsafe {
            plugin.activate.unwrap()(plugin, 48000.0, 64, 4096);
            plugin.start_processing.unwrap()(plugin);
        };
        self.process_start_p = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if !self.process_start_p {
            return Ok(());
        }
        let plugin = unsafe { &*(self.plugin.unwrap()) };
        unsafe {
            plugin.stop_processing.unwrap()(plugin);
            plugin.deactivate.unwrap()(plugin);
        };
        self.process_start_p = false;
        Ok(())
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        let _ = self.gui_close();
        let _ = self.stop();
        if let Some(plugin) = self.plugin {
            let plugin = unsafe { &*plugin };
            unsafe { plugin.destroy.unwrap()(plugin) };
        }
    }
}

struct EventList {
    events: Vec<*const clap_event_header>,
    clap_input_events: clap_input_events,
}

impl EventList {
    pub fn new() -> Self {
        Self {
            events: vec![],
            clap_input_events: clap_input_events {
                ctx: null_mut(),
                size: Some(Self::size),
                get: Some(Self::get),
            },
        }
    }

    pub fn as_clap_input_events(&mut self) -> &clap_input_events {
        self.clap_input_events.ctx = self as *mut _ as *mut c_void;
        &self.clap_input_events
    }

    extern "C" fn size(list: *const clap_input_events) -> u32 {
        let this = unsafe { &*((*list).ctx as *const Self) };
        //log::debug!("EventList size {}", this.events.len() as u32);
        this.events.len() as u32
    }

    extern "C" fn get(list: *const clap_input_events, index: u32) -> *const clap_event_header {
        log::debug!("EventList get {index}");
        let this = unsafe { &*((*list).ctx as *const Self) };
        this.events
            .get(index as usize)
            .copied()
            .unwrap_or(std::ptr::null())
    }

    #[allow(dead_code)]
    pub fn note_on(&mut self, key: i16, channel: i16, velocity: f64, time: u32) {
        let event = Box::new(clap_event_note {
            header: clap_event_header {
                size: size_of::<clap_event_note>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_NOTE_ON,
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel,
            key,
            velocity,
        });
        self.events
            .push(Box::into_raw(event) as *const clap_event_header);
    }

    #[allow(dead_code)]
    pub fn note_off(&mut self, key: i16, channel: i16, velocity: f64, time: u32) {
        let event = Box::new(clap_event_note {
            header: clap_event_header {
                size: size_of::<clap_event_note>() as u32,
                time,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_NOTE_OFF,
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel,
            key,
            velocity,
        });
        self.events
            .push(Box::into_raw(event) as *const clap_event_header);
    }

    fn clear(&mut self) {
        for &ptr in &self.events {
            if !ptr.is_null() {
                unsafe {
                    match (*ptr).type_ {
                        CLAP_EVENT_NOTE_ON
                        | CLAP_EVENT_NOTE_OFF
                        | CLAP_EVENT_NOTE_CHOKE
                        | CLAP_EVENT_NOTE_END => {
                            drop(Box::from_raw(ptr as *mut clap_event_note));
                        }
                        CLAP_EVENT_MIDI => {
                            drop(Box::from_raw(ptr as *mut clap_event_midi));
                        }
                        _ => {
                            unreachable!();
                        }
                    }
                }
            }
        }
        self.events.clear();
    }
}

impl Drop for EventList {
    fn drop(&mut self) {
        self.clear();
    }
}

struct EventListOutput {
    events: Vec<*const clap_event_header>,
    clap_output_events: clap_output_events,
}

impl EventListOutput {
    pub fn new() -> Self {
        Self {
            events: vec![],
            clap_output_events: clap_output_events {
                ctx: null_mut(),
                try_push: Some(Self::try_push),
            },
        }
    }

    pub fn as_clap_output_events(&mut self) -> &clap_output_events {
        self.clap_output_events.ctx = self as *mut _ as *mut c_void;
        &self.clap_output_events
    }

    extern "C" fn try_push(
        list: *const clap_output_events,
        event: *const clap_event_header,
    ) -> bool {
        let this = unsafe { &mut *((*list).ctx as *mut Self) };
        let copied = unsafe { *event }; // shallow copy of header
        log::debug!("EventListOutput try_push {:?}", copied);
        this.events.push(Box::into_raw(Box::new(copied)));
        true
    }

    fn clear(&mut self) {
        for &ptr in &self.events {
            if !ptr.is_null() {
                unsafe {
                    match (*ptr).type_ {
                        CLAP_EVENT_NOTE_ON
                        | CLAP_EVENT_NOTE_OFF
                        | CLAP_EVENT_NOTE_CHOKE
                        | CLAP_EVENT_NOTE_END => {
                            drop(Box::from_raw(ptr as *mut clap_event_note));
                        }
                        CLAP_EVENT_MIDI => {
                            drop(Box::from_raw(ptr as *mut clap_event_midi));
                        }
                        _ => {
                            log::warn!("EventListOutput clear type {:?}", (*ptr).type_);
                            drop(Box::from_raw(ptr as *mut clap_event_header));
                        }
                    }
                }
            }
        }
        self.events.clear();
    }
}

impl Drop for EventListOutput {
    fn drop(&mut self) {
        self.clear();
    }
}
