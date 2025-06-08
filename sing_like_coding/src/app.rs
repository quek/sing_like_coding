use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use anyhow::Result;
use common::protocol::MainToPlugin;
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::app_state::{AppState, AppStateCommand};
use crate::communicator::Communicator;
use crate::device::Device;
use crate::singer::{Singer, SingerCommand};
use crate::view::main_view::MainView;

pub fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 1200.0]),
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
    state: Arc<Mutex<AppState>>,
    device: Option<Device>,
    view: MainView,
    song_sender: Sender<AppStateCommand>,
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
        let singer = Arc::new(Mutex::new(Singer::new(
            song_sender.clone(),
            sender_to_loop.clone(),
        )));
        Singer::start_listener(singer.clone(), view_receiver);
        let mut device = Device::open_default(singer).unwrap();
        device.start().unwrap();
        let device = Some(device);
        view_sender.send(SingerCommand::Song).unwrap();

        let app_state = Arc::new(Mutex::new(AppState::new(view_sender, sender_to_loop)));
        let view = MainView::new(app_state.clone(), song_receiver);

        let app_state_cloned = app_state.clone();
        tokio::spawn(async move {
            dbg!("########## before send_to_plugin_process");
            send_to_plugin_process(app_state_cloned, receiver_from_main)
                .await
                .unwrap();
        });
        Self {
            state: app_state,
            device,
            view,
            song_sender,
        }
    }
}

impl eframe::App for AppMain {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.song_sender.send(AppStateCommand::Quit).unwrap();
        self.state
            .lock()
            .unwrap()
            .sender_to_loop
            .send(MainToPlugin::Quit)
            .unwrap();
        log::debug!("#### on_exit did send MainToPlugin::Quit");
        sleep(Duration::from_millis(100));
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.state.lock().unwrap().gui_context.is_none() {
            self.state.lock().unwrap().gui_context = Some(ctx.clone());
        }
        if self.state.lock().unwrap().hwnd == 0 {
            self.state.lock().unwrap().hwnd = get_hwnd(frame);
        }
        let _ = self.view.view(ctx, &mut self.device);
    }
}

async fn send_to_plugin_process(
    state: Arc<Mutex<AppState>>,
    receiver_from_main: Receiver<MainToPlugin>,
) -> Result<()> {
    let mut plugin_comminicator = Communicator::new(state, receiver_from_main).await?;
    plugin_comminicator.run().await?;

    Ok(())
}

fn get_hwnd(frame: &eframe::Frame) -> isize {
    if let Ok(window_handle) = frame.window_handle() {
        if let RawWindowHandle::Win32(h) = window_handle.as_raw() {
            return isize::from(h.hwnd);
        }
    }
    0
}
