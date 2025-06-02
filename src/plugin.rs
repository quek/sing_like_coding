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
        clap_event_header, clap_event_transport, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_TRANSPORT,
        CLAP_TRANSPORT_HAS_TEMPO, CLAP_TRANSPORT_IS_PLAYING,
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

use crate::{event::Event, singer::ClapPluginPtr, view::main_view::ViewMsg};
use crate::{
    event_list::{EventListInput, EventListOutput},
    process_track_context::ProcessTrackContext,
};

mod window;

pub struct Plugin {
    clap_host: clap_host,
    lib: Option<Library>,
    pub plugin: Option<*const clap_plugin>,
    gui: Option<*const clap_plugin_gui>,
    gui_open_p: bool,
    window_handler: Option<*mut c_void>,
    process_start_p: bool,
    sender_to_view: Sender<ViewMsg>,
    event_list_input: Pin<Box<EventListInput>>,
    event_list_output: Pin<Box<EventListOutput>>,
    host_audio_ports: clap_host_audio_ports,
    host_gui: clap_host_gui,
    host_latency: clap_host_latency,
    host_log: clap_host_log,
    host_params: clap_host_params,
}

macro_rules! cstr {
    ($str:literal) => {
        unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(concat!($str, "\0").as_bytes()) }
    };
}

pub const NAME: &CStr = cstr!("Sing Like Coding");
pub const VENDER: &CStr = cstr!("Sing Like Coding");
pub const URL: &CStr = cstr!("https://github.com/quek/sing_like_coding");
pub const VERSION: &CStr = cstr!("0.0.1");

impl Plugin {
    pub fn new(sender_to_view: Sender<ViewMsg>) -> Pin<Box<Self>> {
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
            sender_to_view,
            event_list_input: EventListInput::new(),
            event_list_output: EventListOutput::new(),
            host_audio_ports,
            host_gui,
            host_latency,
            host_log,
            host_params,
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
        this.sender_to_view
            .send(ViewMsg::PluginCallback(ClapPluginPtr(plugin)))
            .unwrap();
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

    pub fn load(&mut self, path: &Path, index: u32) {
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

            let descriptor = factory.get_plugin_descriptor.unwrap()(factory, index);
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
        log::debug!("gui_open");
        if self.gui_open_p || !self.gui_available() {
            log::debug!("gui_open not gui_available");
            return Ok(());
        }
        log::debug!("gui_open did gui_available");
        let plugin = unsafe { &*(self.plugin.unwrap()) };
        let gui = unsafe { &*self.gui.unwrap() };
        unsafe {
            if !gui.is_api_supported.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false) {
                log::debug!("GUI API not supported");
                return Ok(());
            }

            let is_floating = false;
            log::debug!("GUI API before create");
            if gui.create.unwrap()(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), is_floating) == false {
                panic!("GUI create failed");
            }
            log::debug!("gui_open did create");

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

            let window_handler = create_handler(resizable, width, height, self.clap_host.host_data);
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
        self.gui_open_p = false;
        let plugin = unsafe { &*(self.plugin.unwrap()) };
        let gui = unsafe { &*self.gui.unwrap() };
        unsafe {
            gui.hide.unwrap()(plugin);
            gui.destroy.unwrap()(plugin);
            destroy_handler(self.window_handler.take().unwrap());
        }
        Ok(())
    }

    pub fn process(&mut self, context: &mut ProcessTrackContext) -> Result<()> {
        //log::debug!("plugin.process frames_count {frames_count}");

        let mut in_buf0 = vec![0.0; context.nframes];
        let mut in_buf1 = vec![0.0; context.nframes];
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
        for channel in context.buffer.buffer.iter_mut() {
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

        let transport = if context.play_p {
            Some(clap_event_transport {
                header: clap_event_header {
                    size: size_of::<clap_event_transport>() as u32,
                    time: 0,
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: CLAP_EVENT_TRANSPORT,
                    flags: 0,
                },
                flags: CLAP_TRANSPORT_HAS_TEMPO | CLAP_TRANSPORT_IS_PLAYING,
                song_pos_beats: 0,
                song_pos_seconds: 0,
                tempo: context.bpm,
                tempo_inc: 0.0,
                loop_start_beats: 0,
                loop_end_beats: 0,
                loop_start_seconds: 0,
                loop_end_seconds: 0,
                bar_start: 0,
                bar_number: 0,
                tsig_num: 4,
                tsig_denom: 4,
            })
        } else {
            None
        };

        for event in context.event_list_input.iter() {
            let channel = 0;
            let time = 0;
            match event {
                Event::NoteOn(key, velocity) => {
                    if let Some(key) = context.on_key {
                        self.event_list_input.note_off(key, channel, 0.0, time)
                    }
                    self.event_list_input
                        .note_on(*key, channel, *velocity, time);
                    context.on_key = Some(*key);
                }
                Event::NoteOff(key) => {
                    self.event_list_input.note_off(*key, channel, 0.0, time);
                }
            }
        }
        let in_events = self.event_list_input.as_clap_input_events();
        let out_events = self.event_list_output.as_clap_output_events();
        let prc = clap_process {
            steady_time: context.steady_time,
            frames_count: context.nframes as u32,
            transport: transport.map(|x| &x as *const _).unwrap_or(null()),
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
        if status == CLAP_PROCESS_ERROR {
            panic!("process returns CLAP_PROCESS_ERROR");
        }

        // TODO out_events
        self.event_list_input.clear();
        self.event_list_output.clear();

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

        let entry: Symbol<*const clap_plugin_entry> = unsafe {
            self.lib
                .as_ref()
                .unwrap()
                .get(b"clap_entry\0")
                .expect("Missing symbol")
        };
        let entry = unsafe { &**entry };
        unsafe { entry.deinit.unwrap()() };
    }
}
