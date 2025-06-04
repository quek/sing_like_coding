use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Result;
use common::protocol::{MainToPlugin, PluginToMain};
use common::{to_pcwstr, PIPE_BUFFER_SIZE, PIPE_NAME};
use eframe::egui;
use windows::core::PCWSTR;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::{Foundation::INVALID_HANDLE_VALUE, Storage::FileSystem::PIPE_ACCESS_DUPLEX};

use crate::device::Device;
use crate::plugin_comminicator::PluginCommunicator;
use crate::singer::{Singer, SingerMsg};
use crate::view::main_view::MainView;

pub fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 640.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Sing Like Coding",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(AppMain::default()))
        }),
    );

    result
}

struct AppMain {
    device: Option<Device>,
    view: MainView,
    sender_to_loop: Sender<MainToPlugin>,
    receiver_from_loop: Receiver<PluginToMain>,
}

pub enum Msg {
    Process,
    DidProcess(Vec<Vec<f32>>),
}

impl Default for AppMain {
    fn default() -> Self {
        let (song_sender, song_receiver) = channel();
        let (view_sender, view_receiver) = channel();
        let singer = Arc::new(Mutex::new(Singer::new(song_sender)));
        Singer::start_listener(singer.clone(), view_receiver);
        let mut device = Device::open_default(singer).unwrap();
        device.start().unwrap();
        let device = Some(device);
        view_sender.send(SingerMsg::Song).unwrap();

        let (sender_to_loop, receiver_from_main) = channel();
        let (sender_to_main, receiver_from_loop) = channel();
        let view = MainView::new(view_sender, song_receiver, sender_to_loop.clone());
        thread::spawn(move || {
            dbg!("########## before send_to_plugin_process");
            send_to_plugin_process(sender_to_main, receiver_from_main).unwrap();
        });
        Self {
            device,
            view,
            sender_to_loop,
            receiver_from_loop,
        }
    }
}

impl eframe::App for AppMain {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let _ = self.view.view(ctx, &mut self.device);
    }
}

fn send_to_plugin_process(
    sender_to_main: Sender<PluginToMain>,
    receiver_from_main: Receiver<MainToPlugin>,
) -> Result<()> {
    dbg!("########## in send_to_plugin_process");
    let pipe_name = to_pcwstr(PIPE_NAME);

    unsafe {
        // Named Pipe作成
        dbg!("########## before CreateNamedPipeW");
        let pipe = CreateNamedPipeW(
            PCWSTR(pipe_name.as_ptr()),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
            1,
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            0,
            None,
        );

        if pipe == INVALID_HANDLE_VALUE {
            panic!("Failed to create named pipe");
        }

        dbg!("########## before Command::new(\"sing_like_coding_plugin.exe\")");
        let _child = Command::new("sing_like_coding_plugin.exe")
            .stdout(Stdio::inherit())
            .spawn()
            .expect("Failed to start plugin");

        dbg!("########## before ConnectNamedPipe");
        ConnectNamedPipe(pipe, None)?;

        let mut plugin_comminicator =
            PluginCommunicator::new(pipe, sender_to_main, receiver_from_main);
        dbg!("########## before plugin_comminicator.run()");
        plugin_comminicator.run()?;
        dbg!("########## after plugin_comminicator.run()");

        Ok(())
    }
}
