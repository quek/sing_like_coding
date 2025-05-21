use crate::device::Device;
use crate::plugin;
use crate::plugin::Host;
use eframe::egui;

pub fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
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
    name: String,
    age: u32,
    device: Option<Device>,
    host: Option<Host>,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "Arthur".to_owned(),
            age: 42,
            device: None,
            host: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Application");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Increment").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));

            // ui.image(egui::include_image!(
            //     "../../../crates/egui/assets/ferris.png"
            // ));

            ui.label(format!(
                "Frams per buffer {}",
                self.device
                    .as_ref()
                    .map(|x| x.frames_per_buffer.lock().unwrap().to_string())
                    .unwrap_or("--".to_string())
            ));
            if ui.button("device open").clicked() {
                self.device = Some(Device::open_default().unwrap());
            }
            if ui.button("device start").clicked() {
                self.device.as_mut().unwrap().start().unwrap();
            }
            if ui.button("device stop").clicked() {
                self.device.as_mut().unwrap().stop().unwrap();
            }

            if ui.button("Surge XT").clicked() {
                let frames_per_buffer = self.device.as_ref().unwrap().frames_per_buffer.clone();
                self.host = Some(plugin::foo(frames_per_buffer));
            }
        });
    }
}
