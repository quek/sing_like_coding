use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use common::protocol::{MainToPlugin, PluginToMain};
use eframe::egui;

use crate::comminicator::Communicator;
use crate::device::Device;
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
        let (sender_to_loop, receiver_from_main) = channel();
        let (sender_to_main, receiver_from_loop) = channel();
        let singer = Arc::new(Mutex::new(Singer::new(song_sender, sender_to_loop.clone())));
        Singer::start_listener(singer.clone(), view_receiver);
        let mut device = Device::open_default(singer).unwrap();
        device.start().unwrap();
        let device = Some(device);
        view_sender.send(SingerMsg::Song).unwrap();

        let view = MainView::new(view_sender, song_receiver, sender_to_loop.clone());
        tokio::spawn(async move {
            dbg!("########## before send_to_plugin_process");
            send_to_plugin_process(sender_to_main, receiver_from_main)
                .await
                .unwrap();
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
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let _ = self.sender_to_loop.send(MainToPlugin::Quit);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let _ = self.view.view(ctx, &mut self.device);
    }
}

async fn send_to_plugin_process(
    sender_to_main: Sender<PluginToMain>,
    receiver_from_main: Receiver<MainToPlugin>,
) -> Result<()> {
    dbg!("########## in send_to_plugin_process");

    let mut plugin_comminicator = Communicator::new(sender_to_main, receiver_from_main).await?;
    plugin_comminicator.run().await?;

    Ok(())
}
