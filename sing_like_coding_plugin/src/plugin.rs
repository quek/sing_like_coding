use std::{
    collections::BTreeMap,
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
        CLAP_TRANSPORT_HAS_BEATS_TIMELINE, CLAP_TRANSPORT_HAS_SECONDS_TIMELINE,
        CLAP_TRANSPORT_HAS_TEMPO, CLAP_TRANSPORT_HAS_TIME_SIGNATURE, CLAP_TRANSPORT_IS_LOOP_ACTIVE,
        CLAP_TRANSPORT_IS_PLAYING,
    },
    ext::{
        audio_ports::{
            clap_audio_port_info, clap_host_audio_ports, clap_plugin_audio_ports,
            CLAP_EXT_AUDIO_PORTS,
        },
        gui::{
            clap_host_gui, clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI,
            CLAP_WINDOW_API_WIN32,
        },
        latency::{clap_host_latency, clap_plugin_latency, CLAP_EXT_LATENCY},
        log::{
            clap_host_log, clap_log_severity, CLAP_EXT_LOG, CLAP_LOG_DEBUG, CLAP_LOG_ERROR,
            CLAP_LOG_INFO, CLAP_LOG_WARNING,
        },
        params::{
            clap_host_params, clap_param_clear_flags, clap_param_info, clap_param_rescan_flags,
            clap_plugin_params, CLAP_EXT_PARAMS,
        },
        state::{clap_plugin_state, CLAP_EXT_STATE},
    },
    factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID},
    host::clap_host,
    id::clap_id,
    plugin::clap_plugin,
    process::{clap_process, CLAP_PROCESS_ERROR},
    version::{clap_version_is_compatible, CLAP_VERSION},
};
use common::{
    cstr,
    plugin::param::Param,
    process_data::{EventKind, ProcessData, MAX_PORTS},
};
use libloading::{Library, Symbol};
use stream::{IStream, OStream};
use window::{create_handler, destroy_handler, resize};

use crate::{
    event_list::{EventListInput, EventListOutput},
    plugin_ptr::PluginPtr,
};

mod stream;
mod window;

pub struct Plugin {
    clap_host: clap_host,
    lib: Option<Library>,
    pub plugin: *const clap_plugin,
    ext_audio_ports: Option<*const clap_plugin_audio_ports>,
    ext_gui: Option<*const clap_plugin_gui>,
    ext_latency: Option<*const clap_plugin_latency>,
    ext_params: Option<*const clap_plugin_params>,
    ext_state: Option<*const clap_plugin_state>,
    pub gui_open_p: bool,
    window_handler: Option<*mut c_void>,
    process_start_p: bool,
    sender_to_view: Sender<PluginPtr>,
    audio_port_info_inputs: Vec<clap_audio_port_info>,
    audio_port_info_outputs: Vec<clap_audio_port_info>,
    event_list_input: Pin<Box<EventListInput>>,
    event_list_output: Pin<Box<EventListOutput>>,
    host_audio_ports: clap_host_audio_ports,
    host_gui: clap_host_gui,
    host_latency: clap_host_latency,
    host_log: clap_host_log,
    host_params: clap_host_params,
    hwnd: isize,
    params: BTreeMap<clap_id, Param>,

    next_clock_sample: f64,
    play_p: bool,
}

pub const NAME: &CStr = cstr!("Sing Like Coding");
pub const VENDER: &CStr = cstr!("Sing Like Coding");
pub const URL: &CStr = cstr!("https://github.com/quek/sing_like_coding");
pub const VERSION: &CStr = cstr!("0.0.1");

impl Plugin {
    pub fn new(sender_to_view: Sender<PluginPtr>, hwnd: isize) -> Pin<Box<Self>> {
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
            plugin: null(),
            ext_audio_ports: None,
            ext_gui: None,
            ext_latency: None,
            ext_params: None,
            ext_state: None,
            gui_open_p: false,
            window_handler: None,
            process_start_p: false,
            sender_to_view,
            audio_port_info_inputs: vec![],
            audio_port_info_outputs: vec![],
            event_list_input: EventListInput::new(),
            event_list_output: EventListOutput::new(),
            host_audio_ports,
            host_gui,
            host_latency,
            host_log,
            host_params,
            hwnd,
            params: Default::default(),

            next_clock_sample: 0.0,
            play_p: false,
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
        host: *const clap_host,
        width: u32,
        height: u32,
    ) -> bool {
        let this = unsafe { &mut *((*host).host_data as *mut Self) };
        if let Some(hwnd) = this.window_handler {
            let _ = resize(hwnd, width, height);
        }
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

    unsafe extern "C" fn params_rescan(host: *const clap_host, _flags: clap_param_rescan_flags) {
        log::debug!("params_rescan start");
        let this = unsafe { &mut *((*host).host_data as *mut Self) };
        let _ = this.params();
        log::debug!("params_rescan end");
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
        log::debug!("request_callback start...");
        let this = unsafe { &mut *((*host).host_data as *mut Self) };
        this.sender_to_view
            .send(PluginPtr((unsafe { *host }).host_data))
            .unwrap();
        log::debug!("request_callback end");
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

            let audio_ports = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_AUDIO_PORTS.as_ptr())
                as *const clap_plugin_audio_ports;
            if !audio_ports.is_null() {
                self.ext_audio_ports = Some(audio_ports);
            }

            let gui = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_GUI.as_ptr())
                as *const clap_plugin_gui;
            if !gui.is_null() {
                self.ext_gui = Some(gui);
            }

            let latency = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_LATENCY.as_ptr())
                as *const clap_plugin_latency;
            if !latency.is_null() {
                self.ext_latency = Some(latency);
            }

            let params = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_PARAMS.as_ptr())
                as *const clap_plugin_params;
            if !params.is_null() {
                self.ext_params = Some(params);
            }

            let state = (plugin.get_extension.unwrap())(plugin, CLAP_EXT_STATE.as_ptr())
                as *const clap_plugin_state;
            if !state.is_null() {
                self.ext_state = Some(state);
            }

            self.plugin = clap_plugin;

            self.audio_ports().unwrap();
            self.params().unwrap();
        }
    }

    pub fn audio_ports(&mut self) -> Result<()> {
        self.audio_port_info_inputs.clear();
        self.audio_port_info_outputs.clear();
        unsafe {
            let Some(ext_audio_ports) = self.ext_audio_ports else {
                return Ok(());
            };
            let ext_audio_ports = &*ext_audio_ports;
            let Some(count) = ext_audio_ports.count else {
                return Ok(());
            };
            let Some(get) = ext_audio_ports.get else {
                return Ok(());
            };

            for (is_input, xs) in [
                (true, &mut self.audio_port_info_inputs),
                (false, &mut self.audio_port_info_outputs),
            ] {
                for i in 0..count(self.plugin, is_input) {
                    let mut info = std::mem::zeroed::<clap_audio_port_info>();
                    if get(self.plugin, i, is_input, &mut info) {
                        log::debug!(
                            "{} {} {} {}ch",
                            if is_input { "入力" } else { "出力" },
                            i,
                            CStr::from_ptr(info.name.as_ptr()).to_string_lossy(),
                            info.channel_count
                        );
                        xs.push(info);
                    } else {
                        log::warn!(
                            "Failed to get audio port info at index {} (is_input = {})",
                            i,
                            is_input
                        );
                    }
                }
            }
        }
        if self.audio_port_info_inputs.len() > MAX_PORTS {
            log::warn!(
                "self.audio_port_info_inputs.len() {} > MAX_PORTS {}",
                self.audio_port_info_inputs.len(),
                MAX_PORTS
            );
        }
        if self.audio_port_info_outputs.len() > MAX_PORTS {
            log::warn!(
                "self.audio_port_info_outputs.len() {} > MAX_PORTS {}",
                self.audio_port_info_outputs.len(),
                MAX_PORTS
            );
        }
        Ok(())
    }

    pub fn gui_available(&self) -> bool {
        if self.ext_gui.is_none() {
            return false;
        }
        let gui = unsafe { &*self.ext_gui.unwrap() };
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
        let plugin = unsafe { &*self.plugin };
        let gui = unsafe { &*self.ext_gui.unwrap() };
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

            let window_handler = create_handler(
                resizable,
                width,
                height,
                self.clap_host.host_data,
                self.hwnd,
            );
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
        let plugin = unsafe { &*self.plugin };
        let gui = unsafe { &*self.ext_gui.unwrap() };
        unsafe {
            gui.hide.unwrap()(plugin);
            gui.destroy.unwrap()(plugin);
            destroy_handler(self.window_handler.take().unwrap());
        }
        Ok(())
    }

    pub fn gui_size(&self, width: u32, height: u32) -> Result<()> {
        let gui = unsafe { &*self.ext_gui.unwrap() };
        unsafe { gui.set_size.unwrap()(self.plugin, width, height) };
        Ok(())
    }

    pub fn latency(&self) -> Option<u32> {
        unsafe {
            let ext_latency = &*self.ext_latency?;
            let get = ext_latency.get?;
            Some(get(self.plugin))
        }
    }

    pub fn params(&mut self) -> Result<Vec<Param>> {
        unsafe {
            let plugin = &*self.plugin;
            let ext_params = &*self.ext_params.unwrap();

            let count = ext_params.count.unwrap()(plugin);
            self.params.clear();

            for param_index in 0..count {
                let mut param_info: clap_param_info = std::mem::zeroed();
                if !ext_params.get_info.unwrap()(plugin, param_index, &mut param_info) {
                    continue;
                }

                let name = CStr::from_ptr(param_info.name.as_ptr())
                    .to_string_lossy()
                    .into_owned();
                let module = CStr::from_ptr(param_info.module.as_ptr())
                    .to_string_lossy()
                    .into_owned();

                let mut value = 0.0;
                ext_params.get_value.unwrap()(plugin, param_info.id, &mut value);

                self.params.insert(
                    param_info.id,
                    Param {
                        id: param_info.id,
                        flags: param_info.flags,
                        name,
                        module,
                        min_value: param_info.min_value,
                        max_value: param_info.max_value,
                        default_value: param_info.default_value,
                        value,
                    },
                );
            }

            Ok(self.params.values().cloned().collect::<Vec<_>>())
        }
    }

    pub fn process(&mut self, context: &mut ProcessData) -> Result<()> {
        context.nports_in = self.audio_port_info_inputs.len().min(MAX_PORTS);
        context.nports_out = self.audio_port_info_outputs.len().min(MAX_PORTS);
        for port in 0..context.nports_in {
            context.nchannels_in[port] = self.audio_port_info_inputs[port].channel_count as usize;
        }
        for port in 0..context.nports_out {
            context.nchannels_out[port] = self.audio_port_info_outputs[port].channel_count as usize;
        }

        let mut audio_inputs = Vec::with_capacity(context.nports_in);
        let mut buffer_keeps = vec![];
        for port in 0..context.nports_in {
            let mut in_buffer = vec![];
            for channel in 0..context.nchannels_in[port] {
                in_buffer.push(context.buffer_in[port][channel].as_mut_ptr());
            }
            let audio_input = clap_audio_buffer {
                data32: in_buffer.as_mut_ptr(),
                data64: null_mut::<*mut f64>(),
                channel_count: context.nchannels_in[port] as u32,
                latency: 0,
                constant_mask: context.constant_mask_in[port],
            };
            audio_inputs.push(audio_input);
            buffer_keeps.push(in_buffer);
        }

        let mut audio_outputs = Vec::with_capacity(context.nports_out);
        for port in 0..context.nports_out {
            let mut out_buffer = vec![];
            for channel in 0..context.nchannels_out[port] {
                out_buffer.push(context.buffer_out[port][channel].as_mut_ptr());
            }
            let audio_output = clap_audio_buffer {
                data32: out_buffer.as_mut_ptr(),
                data64: null_mut::<*mut f64>(),
                channel_count: context.nchannels_out[port] as u32,
                latency: 0,
                constant_mask: 0,
            };
            audio_outputs.push(audio_output);
            buffer_keeps.push(out_buffer);
        }

        let mut transport_flags = CLAP_TRANSPORT_HAS_TEMPO
            | CLAP_TRANSPORT_HAS_BEATS_TIMELINE
            | CLAP_TRANSPORT_HAS_SECONDS_TIMELINE
            | CLAP_TRANSPORT_HAS_TIME_SIGNATURE;
        if context.play_p != 0 {
            transport_flags |= CLAP_TRANSPORT_IS_PLAYING;
        }
        if context.loop_p != 0 {
            transport_flags |= CLAP_TRANSPORT_IS_LOOP_ACTIVE;
        }
        let transport = clap_event_transport {
            header: clap_event_header {
                size: size_of::<clap_event_transport>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_TRANSPORT,
                flags: 0,
            },
            flags: transport_flags,
            song_pos_beats: context.song_pos_beats,
            song_pos_seconds: context.song_pos_seconds,
            tempo: context.bpm,
            tempo_inc: 0.0,
            loop_start_beats: context.loop_start_beats,
            loop_end_beats: context.loop_end_beats,
            loop_start_seconds: context.loop_start_seconds,
            loop_end_seconds: context.loop_end_seconds,
            bar_start: context.bar_start,
            bar_number: context.bar_number,
            tsig_num: 4,
            tsig_denom: 4,
        };
        // self.event_list_input.transport(transport.clone());

        let samples_per_delay =
            (context.sample_rate * 60.0) / (context.bpm * context.lpb as f64 * 256.0);
        self.event_list_output.samples_per_delay = samples_per_delay;

        for i in 0..context.nevents_input {
            let event = &context.events_input[i];
            let delay = (event.delay as f64 * samples_per_delay).round() as u32;
            match &event.kind {
                EventKind::NoteOn => {
                    self.event_list_input
                        .note_on(event.key, event.channel, event.velocity, delay)
                }
                EventKind::NoteOff => {
                    self.event_list_input
                        .note_off(event.key, event.channel, event.velocity, delay)
                }
                EventKind::ParamValue => {
                    if self.params.contains_key(&event.param_id) {
                        self.event_list_input
                            .param_value(event.param_id, event.value, delay);
                    }
                }
            }
        }

        {
            if !self.play_p && context.play_p == 1 {
                self.next_clock_sample = 0.0;
                self.event_list_input.midi(0xFA, 0);
            } else if self.play_p && context.play_p == 0 {
                self.event_list_input.midi(0xFC, 0);
            }
            self.play_p = context.play_p == 1;
            if self.play_p {
                let samples_per_clock = context.sample_rate / ((context.bpm / 60.0) * 24.0);
                while self.next_clock_sample < context.nframes as f64 {
                    let frame = self.next_clock_sample as u32;
                    self.event_list_input.midi(0xF8, frame);
                    self.next_clock_sample += samples_per_clock;
                }
                self.next_clock_sample -= context.nframes as f64;
            }
        }

        let in_events = self.event_list_input.as_clap_input_events();
        let out_events = self.event_list_output.as_clap_output_events();

        let prc = clap_process {
            steady_time: context.steady_time,
            frames_count: context.nframes as u32,
            transport: &transport,
            audio_inputs: audio_inputs.as_mut_ptr(),
            audio_outputs: audio_outputs.as_mut_ptr(),
            audio_inputs_count: audio_inputs.len() as u32,
            audio_outputs_count: audio_outputs.len() as u32,
            in_events,
            out_events,
        };

        let plugin = unsafe { &*self.plugin };
        let status = unsafe { plugin.process.unwrap()(plugin, &prc) };
        if status == CLAP_PROCESS_ERROR {
            panic!("process returns CLAP_PROCESS_ERROR");
        }

        // 書き戻す
        for (port, out_buf) in audio_outputs.iter().enumerate() {
            context.constant_mask_out[port] = out_buf.constant_mask;
        }

        self.event_list_input.clear();

        for event in self.event_list_output.events.iter() {
            match event {
                common::event::Event::NoteOn(key, velocity, delay) => {
                    context.output_note_on(*key, *velocity, 0, *delay);
                }
                common::event::Event::NoteOff(key, delay) => {
                    context.output_note_off(*key, 0, *delay);
                }
                common::event::Event::NoteAllOff => { /* 無視 */ }
                common::event::Event::ParamValue(_, param_id, value, delay) => {
                    context.output_param_value(*param_id, *value, *delay);
                }
            }
        }

        self.event_list_output.clear();

        Ok(())
    }

    // pub fn process(&mut self, context: &mut ProcessData) -> Result<()> {
    //     //log::debug!("plugin.process frames_count {frames_count}");

    //     let mut in_buffer = vec![];
    //     for channel in 0..context.nchannels {
    //         in_buffer.push(context.buffer_in[channel].as_mut_ptr());
    //     }
    //     let audio_input = clap_audio_buffer {
    //         data32: in_buffer.as_mut_ptr(),
    //         data64: null_mut::<*mut f64>(),
    //         channel_count: context.nchannels as u32,
    //         latency: 0,
    //         constant_mask: context.constant_mask_in,
    //     };
    //     let mut audio_inputs = [audio_input];

    //     let mut out_buffer = vec![];
    //     for channel in 0..context.nchannels {
    //         out_buffer.push(context.buffer_out[channel].as_mut_ptr());
    //     }

    //     let audio_output = clap_audio_buffer {
    //         data32: out_buffer.as_mut_ptr(),
    //         data64: null_mut::<*mut f64>(),
    //         channel_count: context.nchannels as u32,
    //         latency: 0,
    //         constant_mask: 0,
    //     };
    //     let mut audio_outputs = [audio_output];

    //     let transport = if context.play_p != 0 {
    //         Some(clap_event_transport {
    //             header: clap_event_header {
    //                 size: size_of::<clap_event_transport>() as u32,
    //                 time: 0,
    //                 space_id: CLAP_CORE_EVENT_SPACE_ID,
    //                 type_: CLAP_EVENT_TRANSPORT,
    //                 flags: 0,
    //             },
    //             flags: CLAP_TRANSPORT_HAS_TEMPO | CLAP_TRANSPORT_IS_PLAYING,
    //             song_pos_beats: 0,
    //             song_pos_seconds: 0,
    //             tempo: context.bpm,
    //             tempo_inc: 0.0,
    //             loop_start_beats: 0,
    //             loop_end_beats: 0,
    //             loop_start_seconds: 0,
    //             loop_end_seconds: 0,
    //             bar_start: 0,
    //             bar_number: 0,
    //             tsig_num: 4,
    //             tsig_denom: 4,
    //         })
    //     } else {
    //         None
    //     };

    //     let samples_per_delay =
    //         (context.sample_rate * 60.0) / (context.bpm * context.lpb as f64 * 256.0);
    //     self.event_list_output.samples_per_delay = samples_per_delay;
    //     for i in 0..context.nevents_input {
    //         let event = &context.events_input[i];
    //         match &event.kind {
    //             EventKind::NoteOn => {
    //                 self.event_list_input.note_on(
    //                     event.key,
    //                     event.channel,
    //                     event.velocity,
    //                     (event.delay as f64 * samples_per_delay).round() as u32,
    //                 );
    //             }
    //             EventKind::NoteOff => {
    //                 self.event_list_input.note_off(
    //                     event.key,
    //                     event.channel,
    //                     event.velocity,
    //                     (event.delay as f64 * samples_per_delay).round() as u32,
    //                 );
    //             }
    //             EventKind::ParamValue => {
    //                 if self.params.contains_key(&event.param_id) {
    //                     self.event_list_input.param_value(
    //                         event.param_id,
    //                         event.value,
    //                         (event.delay as f64 * samples_per_delay).round() as u32,
    //                     );
    //                 }
    //             }
    //         }
    //     }
    //     let in_events = self.event_list_input.as_clap_input_events();
    //     let out_events = self.event_list_output.as_clap_output_events();
    //     let prc = clap_process {
    //         steady_time: context.steady_time,
    //         frames_count: context.nframes as u32,
    //         transport: transport.map(|x| &x as *const _).unwrap_or(null()),
    //         audio_inputs: audio_inputs.as_mut_ptr(),
    //         audio_outputs: audio_outputs.as_mut_ptr(),
    //         audio_inputs_count: 1,
    //         audio_outputs_count: 1,
    //         in_events,
    //         out_events,
    //     };
    //     let plugin = unsafe { &*self.plugin };
    //     // log::debug!("before process");
    //     let status = unsafe { plugin.process.unwrap()(plugin, &prc) };
    //     // log::debug!("after process {status}");
    //     if status == CLAP_PROCESS_ERROR {
    //         panic!("process returns CLAP_PROCESS_ERROR");
    //     }

    //     context.constant_mask_out = audio_output.constant_mask;

    //     self.event_list_input.clear();

    //     for event in self.event_list_output.events.iter() {
    //         match event {
    //             common::event::Event::NoteOn(key, velocity, delay) => {
    //                 context.output_note_on(*key, *velocity, 0, *delay);
    //             }
    //             common::event::Event::NoteOff(key, delay) => {
    //                 context.output_note_off(*key, 0, *delay);
    //             }
    //             common::event::Event::NoteAllOff => { /* nothing to do */ }
    //             common::event::Event::ParamValue(_module_index, param_id, value, delay) => {
    //                 context.output_param_value(*param_id, *value, *delay);
    //             }
    //         }
    //     }
    //     self.event_list_output.clear();

    //     Ok(())
    // }

    pub fn start(&mut self) -> Result<()> {
        if self.process_start_p {
            return Ok(());
        }
        let plugin = unsafe { &*self.plugin };
        // let sample_rate = self.supported_stream_config.sample_rate().0 as f64;
        // min_frames_count が 0 だと activate できないみたい
        // let (min_frames_count, max_frames_count): (u32, u32) =
        //     match self.supported_stream_config.buffer_size() {
        //         cpal::SupportedBufferSize::Range { min, max } => (*min, *max),
        //         cpal::SupportedBufferSize::Unknown => (64, 4096),
        //     };
        unsafe {
            // TODO main-thread
            plugin.activate.unwrap()(plugin, 48000.0, 64, 4096);
            // TODO audio-thread
            plugin.start_processing.unwrap()(plugin);
        };
        self.process_start_p = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        if !self.process_start_p {
            return Ok(());
        }
        let plugin = unsafe { &*self.plugin };
        unsafe {
            plugin.stop_processing.unwrap()(plugin);
            plugin.deactivate.unwrap()(plugin);
        };
        self.process_start_p = false;
        Ok(())
    }

    pub fn state_load(&mut self, state: Vec<u8>) -> anyhow::Result<()> {
        let istream = IStream::new(state);
        if let Some(state) = &self.ext_state {
            unsafe {
                let plugin = &*self.plugin;
                let state = &**state;
                state.load.unwrap()(plugin, istream.as_raw());
            }
        }
        Ok(())
    }

    pub fn state_save(&mut self) -> anyhow::Result<Vec<u8>> {
        let ostream = OStream::new();
        if let Some(state) = &self.ext_state {
            unsafe {
                let plugin = &*self.plugin;
                let state = &**state;
                state.save.unwrap()(plugin, ostream.as_raw());
            }
        }
        Ok(ostream.into_inner())
    }
}

impl Drop for Plugin {
    fn drop(&mut self) {
        let _ = self.gui_close();
        let _ = self.stop();
        if !self.plugin.is_null() {
            let plugin = unsafe { &*self.plugin };
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
