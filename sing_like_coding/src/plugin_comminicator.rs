use std::sync::mpsc::{Receiver, Sender};

use common::protocol::{receive, send, MainToPlugin, PluginToMain};
use windows::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::Pipes::DisconnectNamedPipe,
};

pub struct PluginCommunicator {
    pipe: HANDLE,
    sender_to_main: Sender<PluginToMain>,
    receiver_from_main: Receiver<MainToPlugin>,
}

impl PluginCommunicator {
    pub fn new(
        pipe: HANDLE,
        sender_to_main: Sender<PluginToMain>,
        receiver_from_main: Receiver<MainToPlugin>,
    ) -> Self {
        Self {
            pipe,
            sender_to_main,
            receiver_from_main,
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        loop {
            dbg!("before receiver_from_main.recv()?;");
            let message = self.receiver_from_main.recv()?;
            dbg!("after receiver_from_main.recv()?;");
            send(self.pipe, &message)?;

            let message: PluginToMain = receive(self.pipe)?;
            log::debug!("RECEIVED {:?}", message);
            let break_p = message == PluginToMain::Quit;
            self.sender_to_main.send(message)?;
            if break_p {
                break;
            }
        }
        Ok(())
    }
}

impl Drop for PluginCommunicator {
    fn drop(&mut self) {
        unsafe {
            let _ = DisconnectNamedPipe(self.pipe);
            let _ = CloseHandle(self.pipe);
        }
    }
}
