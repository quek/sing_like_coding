use std::sync::mpsc::{channel, Receiver, Sender};

use anyhow::Result;
use common::protocol::{MainToPlugin, PluginToMain};
use windows::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage, MSG, WM_NULL,
    },
};
use windows::Win32::{System::Threading::GetCurrentThreadId, UI::WindowsAndMessaging::PM_REMOVE};

use crate::{clap_manager::ClapManager, host::Host, plugin_ptr::PluginPtr};

pub struct Manager {
    sender_to_loop: Sender<PluginToMain>,
    receiver_from_loop: Receiver<MainToPlugin>,
    sender_from_plugin: Sender<PluginPtr>,
    receiver_from_plugin: Receiver<PluginPtr>,
    plugins: Vec<Vec<Host>>,
    clap_manager: ClapManager,
}

impl Manager {
    pub fn new(
        sender_to_loop: Sender<PluginToMain>,
        receiver_from_loop: Receiver<MainToPlugin>,
    ) -> Self {
        let (sender_from_plugin, receiver_from_plugin) = channel();

        Self {
            sender_to_loop,
            receiver_from_loop,
            sender_from_plugin,
            receiver_from_plugin,
            plugins: vec![],
            clap_manager: ClapManager::new(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let mut win_msg = MSG::default();
        unsafe { PostThreadMessageW(GetCurrentThreadId(), WM_NULL, WPARAM(0), LPARAM(0)) }?;
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
                        if let Some(Some(host)) = self
                            .plugins
                            .get_mut(track_index)
                            .map(|x| x.get_mut(module_index))
                        {
                            if host.plugin.gui_open_p {
                                host.plugin.gui_close()?;
                            } else {
                                host.plugin.gui_open()?;
                            }
                        }
                        self.sender_to_loop.send(PluginToMain::DidGuiOpen)?;
                    }
                    MainToPlugin::Scan => {
                        // TODO scan scan scan
                        self.sender_to_loop.send(PluginToMain::DidScan)?;
                    }
                    MainToPlugin::Quit => {
                        log::debug!("quit");
                        self.sender_to_loop.send(PluginToMain::Quit)?;
                        break;
                    }
                }
            }

            unsafe {
                while PeekMessageW(&mut win_msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&win_msg);
                    let _ = DispatchMessageW(&win_msg);
                }
            };
        }
        Ok(())
    }
}
