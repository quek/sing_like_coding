use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

use crate::clap_manager::ClapManager;
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
    singer: Arc<Mutex<Singer>>,
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
        let mut device = Device::open_default().unwrap();
        device.start(singer.clone()).unwrap();
        let device = Some(device);
        view_sender.send(SingerMsg::Song).unwrap();
        let view = Arc::new(Mutex::new(MainView::new(view_sender)));
        MainView::start_listener(view.clone(), song_receiver);

        Self {
            device,
            singer,
            view,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        {
            let mut singer = self.singer.lock().unwrap();
            if singer.gui_context.is_none() {
                singer.gui_context = Some(ctx.clone());
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Sing Like Coding");

            // Ui.image(egui::include_image!(
            //     "../../../crates/egui/assets/ferris.png"
            // ));

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("device start").clicked() {
                    self.device
                        .as_mut()
                        .unwrap()
                        .start(self.singer.clone())
                        .unwrap();
                }
                if ui.button("device stop").clicked() {
                    self.device.as_mut().unwrap().stop().unwrap();
                }
                if ui.button("Scan CLAP").clicked() {
                    let mut clap_manager = ClapManager::new();
                    clap_manager.scan();
                }
            });

            ui.separator();

            self.view
                .lock()
                .unwrap()
                .view(ui, ctx, &self.singer)
                .unwrap();
        });
    }
}
