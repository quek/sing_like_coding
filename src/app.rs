use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};

use crate::audio_process::AudioProcess;
use crate::device::Device;
use crate::singer::Singer;
use crate::track_view::{TrackView, ViewCommand};
use clap_sys::plugin::clap_plugin;
use eframe::egui;

pub fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 640.0]),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Sawavi",
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
    audio_process: Arc<Mutex<AudioProcess>>,
    singer: Arc<Mutex<Singer>>,
    callback_request_receiver: Receiver<*const clap_plugin>,
    track_view: Arc<Mutex<TrackView>>,
}

pub enum Msg {
    Process,
    DidProcess(Vec<Vec<f32>>),
}

impl Default for MyApp {
    fn default() -> Self {
        let (song_sender, song_receiver) = channel();
        let (view_sender, view_receiver) = channel();
        let (callback_request_sender, callback_request_receiver) = channel();
        let singer = Arc::new(Mutex::new(Singer::new(
            song_sender,
            callback_request_sender,
        )));
        Singer::start_listener(singer.clone(), view_receiver);
        let audio_process = Arc::new(Mutex::new(AudioProcess::new(singer.clone())));
        let mut device = Device::open_default().unwrap();
        device.start(audio_process.clone()).unwrap();
        let device = Some(device);
        view_sender.send(ViewCommand::StateTrack(0)).unwrap();
        let track_view = Arc::new(Mutex::new(TrackView::new(view_sender)));
        TrackView::start_listener(track_view.clone(), song_receiver);

        Self {
            device,
            audio_process,
            singer,
            callback_request_receiver,
            track_view,
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
            loop {
                match self.callback_request_receiver.try_recv() {
                    Ok(plugin) => {
                        let plugin = unsafe { &*plugin };
                        unsafe { plugin.on_main_thread.unwrap()(plugin) };
                        log::debug!("did on_main_thread");
                    }
                    Err(_) => break,
                }
            }
            ui.heading("My egui Application");

            // ui.image(egui::include_image!(
            //     "../../../crates/egui/assets/ferris.png"
            // ));

            ui.separator();

            if ui.button("device start").clicked() {
                self.device
                    .as_mut()
                    .unwrap()
                    .start(self.audio_process.clone())
                    .unwrap();
            }
            if ui.button("device stop").clicked() {
                self.device.as_mut().unwrap().stop().unwrap();
            }

            ui.separator();

            self.track_view.lock().unwrap().view(ui, ctx);
        });
    }
}
