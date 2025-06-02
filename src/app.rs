use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use crate::device::Device;
use crate::singer::{Singer, SingerMsg};
use crate::view::main_view::MainView;
use eframe::egui;

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
            Ok(Box::<MyApp>::default())
        }),
    );

    result
}

struct MyApp {
    device: Option<Device>,
    view: Arc<Mutex<MainView>>,
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
        let view = Arc::new(Mutex::new(MainView::new(view_sender)));
        MainView::start_listener(view.clone(), song_receiver);

        Self { device, view }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.view
            .lock()
            .unwrap()
            .view(ctx, &mut self.device)
            .unwrap();
    }
}
