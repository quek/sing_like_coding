use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use common::protocol::{MainToPlugin, PluginToMain};
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

struct AppMain<'a> {
    state: AppState<'a>,
    device: Option<Device>,
    singer: Arc<Mutex<Singer>>,
    view: MainView,
    song_sender: Sender<AppStateCommand>,
    receiver_main_thread_to_communicator: Option<Receiver<MainToPlugin>>,
    sender_communicator_to_main_thread: Option<Sender<PluginToMain>>,
}

pub enum Msg {
    Process,
    DidProcess(Vec<Vec<f32>>),
}

impl<'a> Default for AppMain<'a> {
    fn default() -> Self {
        let (sender_to_app_state, receiver_from_singer) = channel();
        let (view_sender, view_receiver) = channel();
        let (sender_to_loop, receiver_from_main) = channel();
        let (sender_communicator_to_main_thread, receiver_communicator_to_main_thread) = channel();
        let singer = Arc::new(Mutex::new(Singer::new(
            sender_to_app_state.clone(),
            sender_to_loop.clone(),
        )));
        Singer::start_listener(singer.clone(), view_receiver);

        let mut device = Device::open_default(singer.clone()).unwrap();
        device.start().unwrap();
        let device = Some(device);
        view_sender.send(SingerCommand::Song).unwrap();

        let app_state = AppState::new(
            view_sender,
            sender_to_loop,
            receiver_communicator_to_main_thread,
            receiver_from_singer,
        );
        let view = MainView::new();

        // let app_state_cloned = app_state.clone();
        // tokio::spawn(async move {
        //     dbg!("########## before send_to_plugin_process");
        //     send_to_plugin_process(app_state_cloned, receiver_from_main)
        //         .await
        //         .unwrap();
        // });
        Self {
            state: app_state,
            device,
            singer,
            view,
            song_sender: sender_to_app_state,
            receiver_main_thread_to_communicator: Some(receiver_from_main),
            sender_communicator_to_main_thread: Some(sender_communicator_to_main_thread),
        }
    }
}

impl<'a> eframe::App for AppMain<'a> {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.song_sender.send(AppStateCommand::Quit).unwrap();
        self.state.sender_to_loop.send(MainToPlugin::Quit).unwrap();
        log::debug!("#### on_exit did send MainToPlugin::Quit");
        sleep(Duration::from_millis(100));
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if let Some(receiver_main_thread_to_communicator) =
            self.receiver_main_thread_to_communicator.take()
        {
            let mut communicator = Communicator::new(
                receiver_main_thread_to_communicator,
                self.sender_communicator_to_main_thread.take().unwrap(),
                ctx.clone(),
            )
            .unwrap();
            tokio::spawn(async move {
                communicator.run().await.unwrap();
            });

            self.singer.lock().unwrap().gui_context = Some(ctx.clone());
            self.state.gui_context = Some(ctx.clone());

            self.state.hwnd = get_hwnd(frame);
        }
        let _ = self.view.view(ctx, &mut self.device, &mut self.state);

        // 節電
        let repaint_after = std::time::Duration::from_secs_f64(1.0 / 4.0);
        ctx.request_repaint_after(repaint_after);
    }
}

// async fn send_to_plugin_process(
//     state: Arc<Mutex<AppState>>,
//     receiver_from_main: Receiver<MainToPlugin>,
// ) -> Result<()> {
//     let mut communicator = Communicator::new(state, receiver_from_main).await?;
//     communicator.run().await?;

//     Ok(())
// }

fn get_hwnd(frame: &eframe::Frame) -> isize {
    if let Ok(window_handle) = frame.window_handle() {
        if let RawWindowHandle::Win32(h) = window_handle.as_raw() {
            return isize::from(h.hwnd);
        }
    }
    0
}
