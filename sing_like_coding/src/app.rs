use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use common::protocol::{MainToPlugin, PluginToMain};
use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::app_state::AppState;
use crate::communicator::Communicator;
use crate::device::Device;
use crate::singer::Singer;
use crate::view::root_view::RootView;

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
    view: RootView,
    recevier_from_main_thread: Option<Receiver<MainToPlugin>>,
    sender_communicator_to_main_thread: Option<Sender<PluginToMain>>,
}

pub enum Msg {
    Process,
    DidProcess(Vec<Vec<f32>>),
}

impl<'a> Default for AppMain<'a> {
    fn default() -> Self {
        let (sender_to_main, receiver_from_audio) = channel();
        let (sender_to_singer, recevier_from_ui) = channel();
        let (sender_to_plugin, recevier_from_main_thread) = channel();
        let (sender_communicator_to_main_thread, receiver_communicator_to_main_thread) = channel();
        let (sender_midi, receiver_midi) = channel();
        let singer = Arc::new(Mutex::new(Singer::new(sender_to_main)));
        Singer::start_listener(singer.clone(), recevier_from_ui);
        Singer::start_listener_midi(singer.clone(), receiver_midi);

        let mut device = Device::open_default(singer.clone()).unwrap();
        device.start().unwrap();
        let device = Some(device);

        let app_state = AppState::new(
            singer.lock().unwrap().song.clone(),
            sender_to_singer,
            receiver_from_audio,
            sender_to_plugin,
            receiver_communicator_to_main_thread,
            sender_midi,
        );
        let view = RootView::new();

        Self {
            state: app_state,
            device,
            singer,
            view,
            recevier_from_main_thread: Some(recevier_from_main_thread),
            sender_communicator_to_main_thread: Some(sender_communicator_to_main_thread),
        }
    }
}

impl<'a> eframe::App for AppMain<'a> {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.device.as_mut().map(|x| {
            let _ = x.stop();
        });
        self.state.quit();
        self.state
            .send_to_plugin(MainToPlugin::Quit, Box::new(|_, _| Ok(())))
            .unwrap();
        log::debug!("#### on_exit did send MainToPlugin::Quit");
        sleep(Duration::from_millis(100));
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.state.now_update();
        if let Some(receiver_main_thread_to_communicator) = self.recevier_from_main_thread.take() {
            let hwnd = get_hwnd(frame);
            let mut communicator = Communicator::new(
                receiver_main_thread_to_communicator,
                self.sender_communicator_to_main_thread.take().unwrap(),
                ctx.clone(),
            )
            .unwrap();
            tokio::spawn(async move {
                communicator.run(hwnd).await.unwrap();
            });

            self.singer.lock().unwrap().gui_context = Some(ctx.clone());

            self.state.gui_context = Some(ctx.clone());
        }
        let _ = self.view.view(ctx, &mut self.device, &mut self.state);

        // 節電
        let fps = if self.state.song_state.play_p {
            60.0
        } else {
            4.0
        };
        let repaint_after = std::time::Duration::from_secs_f64(1.0 / fps);
        ctx.request_repaint_after(repaint_after);
    }
}

fn get_hwnd(frame: &eframe::Frame) -> isize {
    if let Ok(window_handle) = frame.window_handle() {
        if let RawWindowHandle::Win32(h) = window_handle.as_raw() {
            return isize::from(h.hwnd);
        }
    }
    unreachable!("get_hwd failed!");
}
