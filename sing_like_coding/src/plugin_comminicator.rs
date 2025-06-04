use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};

use common::protocol::{receive, send, MainToPlugin, PluginToMain};
use common::PIPE_CTRL_NAME;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

pub struct PluginCommunicator {
    child: Child,
    pipe: NamedPipeServer,
    sender_to_main: Sender<PluginToMain>,
    receiver_from_main: Receiver<MainToPlugin>,
}

impl PluginCommunicator {
    pub async fn new(
        sender_to_main: Sender<PluginToMain>,
        receiver_from_main: Receiver<MainToPlugin>,
    ) -> anyhow::Result<Self> {
        let pipe = ServerOptions::new().create(PIPE_CTRL_NAME)?;

        dbg!("########## before Command::new(\"sing_like_coding_plugin.exe\")");
        let child = Command::new("sing_like_coding_plugin.exe")
            .stdout(Stdio::inherit())
            .spawn()
            .expect("Failed to start plugin");

        pipe.connect().await?;

        Ok(Self {
            child,
            pipe,
            sender_to_main,
            receiver_from_main,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            dbg!("before receiver_from_main.recv()?;");
            let message = self.receiver_from_main.recv()?;
            dbg!("after receiver_from_main.recv()?;");
            send(&mut self.pipe, &message).await?;

            let message: PluginToMain = receive(&mut self.pipe).await?;
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
