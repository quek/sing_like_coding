use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use common::protocol::MainToPlugin;
use common::protocol::PluginToMain;

use crate::main_comminicator::MainCommunicator;
use crate::plugin_host::PluginHost;

pub fn main() {
    let (sender_to_loop, receiver_from_main) = channel();
    let (sender_to_main, receiver_from_loop) = channel();
    let mut plugin_host = PluginHost::new(sender_to_loop, receiver_from_loop);
    log::debug!("$$$$$$$ before thread::spawn");
    tokio::spawn(async move {
        log::debug!("$$$$$$$ before receive_from_main_process");
        receive_from_main_process(sender_to_main, receiver_from_main)
            .await
            .unwrap();
    });
    plugin_host.run().unwrap();
}

async fn receive_from_main_process(
    sender_to_main: Sender<MainToPlugin>,
    receiver_from_main: Receiver<PluginToMain>,
) -> anyhow::Result<()> {
    let mut main_comminicator = MainCommunicator::new(sender_to_main, receiver_from_main).await?;
    main_comminicator.run().await?;

    Ok(())
}
