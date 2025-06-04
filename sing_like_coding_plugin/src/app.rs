use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

use common::protocol::MainToPlugin;
use common::protocol::PluginToMain;
use common::to_pcwstr;
use common::PIPE_NAME;
use windows::{
    core::PCWSTR,
    Win32::Foundation::INVALID_HANDLE_VALUE,
    Win32::Security::SECURITY_ATTRIBUTES,
    Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_GENERIC_READ, FILE_GENERIC_WRITE,
        FILE_SHARE_READ, OPEN_EXISTING,
    },
};

use crate::main_comminicator::MainCommunicator;
use crate::plugin_host::PluginHost;

pub fn main() {
    let (sender_to_loop, receiver_from_main) = channel();
    let (sender_to_main, receiver_from_loop) = channel();
    let mut plugin_host = PluginHost::new(sender_to_loop, receiver_from_loop);
    log::debug!("$$$$$$$ before thread::spawn");
    thread::spawn(move || {
        log::debug!("$$$$$$$ before receive_from_main_process");
        receive_from_main_process(sender_to_main, receiver_from_main).unwrap();
    });
    plugin_host.run().unwrap();
}

fn receive_from_main_process(
    sender_to_main: Sender<MainToPlugin>,
    receiver_from_main: Receiver<PluginToMain>,
) -> anyhow::Result<()> {
    let pipe_name = to_pcwstr(PIPE_NAME);

    let pipe = unsafe {
        dbg!("$$$$$$$ before CreateFileW");
        let pipe = CreateFileW(
            PCWSTR(pipe_name.as_ptr()),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            FILE_SHARE_READ,
            Some(&SECURITY_ATTRIBUTES::default()),
            OPEN_EXISTING,
            FILE_FLAGS_AND_ATTRIBUTES(0),
            None,
        )?;
        dbg!("$$$$$$$ after CreateFileW");
        if pipe == INVALID_HANDLE_VALUE {
            panic!("Plugin: Failed to connect to named pipe");
        }
        pipe
    };

    let mut main_comminicator = MainCommunicator::new(pipe, sender_to_main, receiver_from_main);
    dbg!("$$$$$$$ before main_comminicator.run()?;");
    main_comminicator.run()?;

    Ok(())
}
