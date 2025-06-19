use std::{
    collections::HashMap,
    sync::mpsc::{channel, Receiver, Sender},
    thread::{self, sleep},
    time::Duration,
};

use anyhow::Result;
use common::{
    clap_manager::ClapManager,
    protocol::{MainToPlugin, PluginToMain},
    str::to_pcstr,
};
use windows::Win32::{
    Foundation::{HANDLE, LPARAM, WPARAM},
    System::Threading::{CreateEventA, SetEvent},
    UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG, WM_NULL,
    },
};
use windows::Win32::{System::Threading::GetCurrentThreadId, UI::WindowsAndMessaging::PM_REMOVE};

use crate::{host::Host, plugin_ptr::PluginPtr};

pub struct Manager {
    sender_to_loop: Sender<PluginToMain>,
    receiver_from_loop: Receiver<MainToPlugin>,
    sender_from_plugin: Sender<PluginPtr>,
    receiver_from_plugin: Receiver<PluginPtr>,
    event_quit_all: HANDLE,
    hosts: HashMap<usize, Host>,
    clap_manager: ClapManager,
    hwnd: isize,
}

pub const EVENT_QUIT_ALL_NAME: &str = "SingLikeCoding.Plugin.Quit.All";

impl Manager {
    pub fn new(
        sender_to_loop: Sender<PluginToMain>,
        receiver_from_loop: Receiver<MainToPlugin>,
    ) -> anyhow::Result<Self> {
        let (sender_from_plugin, receiver_from_plugin) = channel();
        let (event_quit_all_name, _x) = to_pcstr(EVENT_QUIT_ALL_NAME)?;
        let event_quit_all = unsafe {
            CreateEventA(
                None,
                true.into(),  // 手動リセット
                false.into(), // 初期非シグナル
                event_quit_all_name,
            )?
        };

        Ok(Self {
            sender_to_loop,
            receiver_from_loop,
            sender_from_plugin,
            receiver_from_plugin,
            event_quit_all,
            hosts: Default::default(),
            clap_manager: ClapManager::new(),
            hwnd: 0,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // 最初は窓が一つもないために、これがないと PeekMessageW がエラーになる
        unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_NULL, WPARAM(0), LPARAM(0)) }?;
        let mut win_msg = MSG::default();
        loop {
            if let Ok(message) = self.receiver_from_loop.try_recv() {
                match message {
                    MainToPlugin::Hwnd(hwnd) => {
                        self.hwnd = hwnd;
                        self.sender_to_loop.send(PluginToMain::DidHwnd)?;
                    }
                    MainToPlugin::Load(id, clap_id, gui_open_p) => {
                        log::debug!("will load {id}");
                        let description = self.clap_manager.description(&clap_id).unwrap();
                        let host = Host::new(
                            id,
                            description,
                            self.sender_from_plugin.clone(),
                            gui_open_p,
                            self.hwnd,
                        )?;
                        let latency = host.latency();
                        self.hosts.insert(id, host);

                        self.sender_to_loop
                            .send(PluginToMain::DidLoad(id, latency))?;
                    }
                    MainToPlugin::Unload(id) => {
                        if let Some(host) = self.host(id) {
                            host.unload()?;
                            self.hosts.remove(&id);
                        }
                        self.sender_to_loop.send(PluginToMain::DidUnload(id))?;
                    }
                    MainToPlugin::GuiOpen(id) => {
                        if let Some(host) = self.host(id) {
                            if host.plugin.gui_open_p {
                                host.plugin.gui_close()?;
                            } else {
                                host.plugin.gui_open()?;
                            }
                        }
                        self.sender_to_loop.send(PluginToMain::DidGuiOpen)?;
                    }
                    MainToPlugin::Params(id) => {
                        let mut params = vec![];
                        if let Some(host) = self.host(id) {
                            params = host.params()?;
                        }
                        self.sender_to_loop.send(PluginToMain::DidParams(params))?;
                    }
                    MainToPlugin::StateLoad(id, state) => {
                        if let Some(host) = self.host(id) {
                            host.load(state)?;
                        }
                        self.sender_to_loop.send(PluginToMain::DidStateLoad)?;
                    }
                    MainToPlugin::StateSave(id) => {
                        let state = if let Some(host) = self.host(id) {
                            host.save()?
                        } else {
                            vec![]
                        };
                        self.sender_to_loop
                            .send(PluginToMain::DidStateSave(id, state))?;
                    }
                    MainToPlugin::Scan => {
                        log::debug!("clap_manager.scan() start...");
                        self.clap_manager.scan();
                        log::debug!("clap_manager.scan() end");
                        self.sender_to_loop.send(PluginToMain::DidScan)?;
                    }
                    MainToPlugin::Quit => {
                        log::debug!("$$$$ quit");
                        self.sender_to_loop.send(PluginToMain::Quit)?;
                        unsafe { SetEvent(self.event_quit_all) }?;
                        sleep(Duration::from_millis(1000));
                        return Ok(());
                    }
                }
            }

            if let Ok(plugin_ptr) = self.receiver_from_plugin.try_recv() {
                let plugin = unsafe { plugin_ptr.as_mut() };
                let plugin = unsafe { &*plugin.plugin };
                log::debug!("will on_main_thread");
                unsafe { plugin.on_main_thread.unwrap()(plugin) };
                log::debug!("did on_main_thread");
            }

            unsafe {
                while PeekMessageW(&mut win_msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&win_msg);
                    let _ = DispatchMessageW(&win_msg);
                }
            };

            // plugin.on_main_thread と PeekMessageW は同じスレッドである必要がるため
            // スレッドを分けるのが面倒なためスリープしちゃう
            thread::sleep(Duration::from_millis(1000 / 60));
        }
    }

    fn host(&mut self, id: usize) -> Option<&mut Host> {
        self.hosts.get_mut(&id)
    }
}
