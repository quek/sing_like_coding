use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};

use common::protocol::{receive, send, MainToPlugin, PluginToMain};
use common::PIPE_CTRL_NAME;
use tokio::net::windows::named_pipe::ServerOptions;

pub struct Communicator {
    receiver_from_main: Receiver<MainToPlugin>,
    sender_communicator_to_main_thread: Sender<PluginToMain>,
    gui_context: eframe::egui::Context,
}

impl Communicator {
    pub fn new(
        receiver_from_main: Receiver<MainToPlugin>,
        sender_communicator_to_main_thread: Sender<PluginToMain>,
        gui_context: eframe::egui::Context,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            receiver_from_main,
            sender_communicator_to_main_thread,
            gui_context,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let mut pipe = ServerOptions::new().create(PIPE_CTRL_NAME)?;
        let _child = Command::new("sing_like_coding_plugin.exe")
            .stdout(Stdio::inherit())
            .spawn()?;
        pipe.connect().await?;

        loop {
            let message = self.receiver_from_main.recv()?;
            send(&mut pipe, &message).await?;

            let message: PluginToMain = receive(&mut pipe).await?;
            let break_p = message == PluginToMain::Quit;
            self.sender_communicator_to_main_thread.send(message)?;
            self.gui_context.request_repaint();
            if break_p {
                log::debug!("#### end Communicator run loop.");
                return Ok(());
            }
        }
    }
}
