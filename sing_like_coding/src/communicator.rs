use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use common::protocol::{receive, send, MainToPlugin, PluginToMain};
use common::PIPE_CTRL_NAME;
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

use crate::app_state::AppState;

pub struct Communicator {
    state: Arc<Mutex<AppState>>,
    _child: Child,
    pipe: NamedPipeServer,
    receiver_from_main: Receiver<MainToPlugin>,
}

impl Communicator {
    pub async fn new(
        state: Arc<Mutex<AppState>>,
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
            state,
            _child: child,
            pipe,
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
            self.state
                .lock()
                .unwrap()
                .received_from_plugin_process(message)?;
            if break_p {
                break;
            }
        }
        Ok(())
    }
}
