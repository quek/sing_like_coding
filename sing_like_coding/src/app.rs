use std::process::{Command, Stdio};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;

use common::protocol::{Hello, Protocol};
use common::{to_pcwstr, PIPE_BUFFER_SIZE, PIPE_NAME};
use eframe::egui;
use windows::core::PCWSTR;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_MESSAGE,
    PIPE_TYPE_MESSAGE, PIPE_WAIT,
};
use windows::Win32::{
    Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
    Storage::FileSystem::PIPE_ACCESS_DUPLEX,
};

use crate::device::Device;
use crate::singer::{Singer, SingerMsg};
use crate::view::main_view::MainView;

pub fn main() -> eframe::Result {
    let app = Box::<MyApp>::default();
    thread::spawn(move || {
        plugin_process().unwrap();
    });

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
            Ok(app)
        }),
    );

    result
}

struct MyApp {
    device: Option<Device>,
    view: MainView,
}

pub enum Msg {
    Process,
    DidProcess(Vec<Vec<f32>>),
}

impl Default for MyApp {
    fn default() -> Self {
        let (song_sender, song_receiver) = channel();
        let (view_sender, view_receiver) = channel();
        let singer = Arc::new(Mutex::new(Singer::new(song_sender)));
        Singer::start_listener(singer.clone(), view_receiver);
        let mut device = Device::open_default(singer).unwrap();
        device.start().unwrap();
        let device = Some(device);
        view_sender.send(SingerMsg::Song).unwrap();
        let view = MainView::new(view_sender, song_receiver);

        Self { device, view }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let _ = self.view.view(ctx, &mut self.device);
    }
}

pub fn plugin_process() -> anyhow::Result<()> {
    let pipe_name = to_pcwstr(PIPE_NAME);

    unsafe {
        // Named Pipe作成
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

        // プラグインプロセス起動
        let _child = Command::new("sing_like_coding_plugin.exe")
            .stdout(Stdio::inherit())
            .spawn()
            .expect("Failed to start plugin");

        ConnectNamedPipe(pipe, None)?;

        srloop(pipe)?;

        DisconnectNamedPipe(pipe)?;
        CloseHandle(pipe)?;

        Ok(())
    }
}

fn srloop(pipe: HANDLE) -> anyhow::Result<()> {
    let messages = vec![
        Protocol::Hello(Hello {
            message: "World!".to_string(),
        }),
        Protocol::Quit,
    ];

    for message in &messages {
        message.send(pipe)?;
        let message = Protocol::receive(pipe)?;
        dbg!(message);
    }

    Ok(())
}
