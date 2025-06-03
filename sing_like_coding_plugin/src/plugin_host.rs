use std::{
    path::Path,
    sync::mpsc::{channel, Receiver, Sender},
};

use anyhow::{anyhow, Result};
use clap_sys::ext::track_info;
use common::protocol::{MainToPlugin, PluginToMain};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG,
};

use crate::{clap_manager::ClapManager, plugin::Plugin, process_track_context::PluginPtr};

pub struct PluginHost {
    sender_to_loop: Sender<PluginToMain>,
    receiver_from_loop: Receiver<MainToPlugin>,
    sender_from_plugin: Sender<PluginPtr>,
    receiver_from_plugin: Receiver<PluginPtr>,
    plugins: Vec<Vec<Pin<Box<Plugin>>>>,
    clap_manager: ClapManager,
}

impl PluginHost {
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
        loop {
            if let Ok(message) = self.receiver_from_loop.try_recv() {
                match message {
                    MainToPlugin::Load(id, track_index) => {
                        log::debug!("will load {id}");
                        self.load(id, track_index)?;
                        self.sender_to_loop.send(PluginToMain::DidLoad)?;
                    }
                    MainToPlugin::Quit => {
                        log::debug!("quit");
                        self.sender_to_loop.send(PluginToMain::Quit)?;
                        break;
                    }
                }
            }

            // ウインドウメッセージの取得
            unsafe {
                let ret = GetMessageW(&mut win_msg, None, 0, 0);
                if ret.0 == 0 {
                    // WM_QUIT受信で終了
                    break;
                } else if ret.0 == -1 {
                    // エラー処理
                    return Err(anyhow!("GetMessageW failed"));
                }

                let _ = TranslateMessage(&win_msg);
                let _ = DispatchMessageW(&win_msg);
            };
        }
        Ok(())
    }

    fn load(&mut self, id: String, track_index: usize) -> Result<()> {
        let description = self.clap_manager.description(&id).unwrap();
        let mut plugin = Plugin::new(self.sender_from_plugin.clone());
        plugin.load(Path::new(&description.path), description.index);
        plugin.start()?;
        plugin.gui_open()?;
        loop {
            if self.plugins.len() > track_index {
                break;
            }
            self.plugins.push(vec![]);
        }
        self.plugins[track_index].push(plugin);

        Ok(())
    }
}
