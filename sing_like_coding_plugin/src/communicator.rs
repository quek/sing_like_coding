use std::sync::mpsc::{Receiver, Sender};

use common::{
    protocol::{receive, send, MainToPlugin, PluginToMain},
    PIPE_CTRL_NAME,
};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient};

pub struct Communicator {
    pipe: NamedPipeClient,
    sender_to_main: Sender<MainToPlugin>,
    receiver_from_main: Receiver<PluginToMain>,
}

impl Communicator {
    pub async fn new(
        sender_to_main: Sender<MainToPlugin>,
        receiver_from_main: Receiver<PluginToMain>,
    ) -> anyhow::Result<Self> {
        let pipe = ClientOptions::new().open(PIPE_CTRL_NAME)?;

        Ok(Self {
            pipe,
            sender_to_main,
            receiver_from_main,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let message: MainToPlugin = receive(&mut self.pipe).await?;
            log::debug!("$$$$ DID RECEIVE {:?}", message);
            self.sender_to_main.send(message)?;
            let message = self.receiver_from_main.recv()?;
            send(&mut self.pipe, &message).await?;
            log::debug!("$$$$ DiD SEND {:?}", message);
            if message == PluginToMain::Quit {
                log::debug!("$$$$ end Communicator run loop.");
                return Ok(());
            }
        }
    }
}
