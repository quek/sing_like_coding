use std::sync::mpsc::{Receiver, Sender};

use common::protocol::{receive, send, MainToPlugin, PluginToMain};
use windows::Win32::Foundation::{CloseHandle, HANDLE};

pub struct MainCommunicator {
    pipe: HANDLE,
    sender_to_main: Sender<MainToPlugin>,
    receiver_from_main: Receiver<PluginToMain>,
}

impl MainCommunicator {
    pub fn new(
        pipe: HANDLE,
        sender_to_main: Sender<MainToPlugin>,
        receiver_from_main: Receiver<PluginToMain>,
    ) -> Self {
        Self {
            pipe,
            sender_to_main,
            receiver_from_main,
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let message: MainToPlugin = receive(self.pipe)?;
            log::debug!("RECEIVED {:?}", message);
            self.sender_to_main.send(message);
            let message = self.receiver_from_main.recv()?;
            send(self.pipe, &message);
            if message == PluginToMain::Quit {
                break;
            }
        }
        Ok(())
    }
}

impl Drop for MainCommunicator {
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.pipe) };
    }
}
