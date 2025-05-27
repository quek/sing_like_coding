use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use crate::audio_process::AudioProcess;
use crate::device::Device;
use crate::plugin::Plugin;
use crate::song::Song;
use clap_sys::plugin::clap_plugin;
use eframe::egui;

pub fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([640.0, 480.0]),
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
    plugin: Option<Plugin>,
    audio_process: Arc<Mutex<AudioProcess>>,
    song: Arc<Mutex<Song>>,
    callback_request_sender: Sender<*const clap_plugin>,
    callback_request_receiver: Receiver<*const clap_plugin>,
}

pub enum Msg {
    Process,
    DidProcess(Vec<Vec<f32>>),
}

impl Default for MyApp {
    fn default() -> Self {
        let device = Some(Device::open_default().unwrap());
        let (sender, receiver) = channel();
        let song = Arc::new(Mutex::new(Song::new()));
        let audio_process = AudioProcess::new(song.clone());

        Self {
            device,
            plugin: None,
            audio_process: Arc::new(Mutex::new(audio_process)),
            song,
            callback_request_sender: sender,
            callback_request_receiver: receiver,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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

            if ui.button("Load Surge XT").clicked() {
                let mut plugin = Plugin::new(self.callback_request_sender.clone(), ctx.clone());
                let path =
                    Path::new("c:/Program Files/Common Files/CLAP/Surge Synth Team/Surge XT.clap");
                //let path = Path::new("c:/Program Files/Common Files/CLAP/VCV Rack 2.clap");
                plugin.load(path);
                plugin.start().unwrap();
                plugin.gui_open().unwrap();
                self.song.lock().unwrap().tracks[0].modules.push(plugin);
            }
            if ui.button("Load VCV Rack").clicked() {
                //let path = Path::new("c:/Program Files/Common Files/CLAP/VCV Rack 2.clap");
            }

            ui.separator();

            if ui.button("Surge XT edit").clicked() {
                self.plugin.as_mut().map(|x| x.gui_open());
            }
            if ui.button("Surge XT close").clicked() {
                self.plugin.as_mut().map(|x| x.gui_close());
            }
            if ui.button("Surge XT start").clicked() {
                self.plugin.as_mut().map(|x| x.start());
            }
            if ui.button("Surge XT stop").clicked() {
                self.plugin.as_mut().map(|x| x.stop());
            }
        });
    }
}
