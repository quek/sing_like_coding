use std::{
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
    event_quit: HANDLE,
    plugins: Vec<Vec<Host>>,
    clap_manager: ClapManager,
}

pub const EVENT_QUIT_NAME: &str = "SingLikeCoding.Plugin.Quit";

impl Manager {
    pub fn new(
        sender_to_loop: Sender<PluginToMain>,
        receiver_from_loop: Receiver<MainToPlugin>,
    ) -> anyhow::Result<Self> {
        let (sender_from_plugin, receiver_from_plugin) = channel();
        let (event_quit_name, _x) = to_pcstr(EVENT_QUIT_NAME)?;
        let event_quit = unsafe {
            CreateEventA(
                None,
                true.into(),  // 手動リセット
                false.into(), // 初期非シグナル
                event_quit_name,
            )?
        };

        Ok(Self {
            sender_to_loop,
            receiver_from_loop,
            sender_from_plugin,
            receiver_from_plugin,
            event_quit,
            plugins: vec![],
            clap_manager: ClapManager::new(),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        // 最初は窓が一つもないために、これがないと PeekMessageW がエラーになる
        unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_NULL, WPARAM(0), LPARAM(0)) }?;
        let mut win_msg = MSG::default();
        loop {
            if let Ok(message) = self.receiver_from_loop.try_recv() {
                match message {
                    MainToPlugin::Load(id, clap_id, track_index) => {
                        log::debug!("will load {id}");
                        let description = self.clap_manager.description(&clap_id).unwrap();
                        let host = Host::new(id, description, self.sender_from_plugin.clone())?;
                        loop {
                            if self.plugins.len() > track_index {
                                break;
                            }
                            self.plugins.push(vec![]);
                        }
                        self.plugins[track_index].push(host);

                        self.sender_to_loop.send(PluginToMain::DidLoad)?;
                    }
                    MainToPlugin::GuiOpen(track_index, module_index) => {
                        if let Some(host) = self.host(track_index, module_index) {
                            if host.plugin.gui_open_p {
                                host.plugin.gui_close()?;
                            } else {
                                host.plugin.gui_open()?;
                            }
                        }
                        self.sender_to_loop.send(PluginToMain::DidGuiOpen)?;
                    }
                    MainToPlugin::StateLoad(track_index, module_index, state) => {
                        if let Some(host) = self.host(track_index, module_index) {
                            host.load(state)?;
                        }
                        self.sender_to_loop.send(PluginToMain::DidStateLoad)?;
                    }
                    MainToPlugin::StateSave(track_index, module_index) => {
                        let state = if let Some(host) = self.host(track_index, module_index) {
                            host.save()?
                        } else {
                            vec![]
                        };
                        self.sender_to_loop.send(PluginToMain::DidStateSave(
                            track_index,
                            module_index,
                            state,
                        ))?;
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
                        unsafe { SetEvent(self.event_quit) }?;
                        sleep(Duration::from_millis(1000));
                        return Ok(());
                    }
                }
            }

            if let Ok(plugin_ptr) = self.receiver_from_plugin.try_recv() {
                let plugin = unsafe { plugin_ptr.as_mut() };
                let plugin = unsafe { &*plugin.plugin.unwrap() };
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

    fn host(&mut self, track_index: usize, module_index: usize) -> Option<&mut Host> {
        self.plugins
            .get_mut(track_index)
            .and_then(|x| x.get_mut(module_index))
    }
}
