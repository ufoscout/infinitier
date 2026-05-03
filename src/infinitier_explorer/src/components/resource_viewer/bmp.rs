use eframe::egui;
use infinitier_core::{fs::Importer, game::{GameResource, ResourceId}, resource::bmp::BmpImporter};

pub struct BmpViewer {
    cached: Option<(ResourceId, Result<egui::TextureHandle, String>)>,
}

impl BmpViewer {
    pub fn new() -> Self {
        Self { cached: None }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, resource_id: ResourceId, resource: &GameResource) {
        if self.cached.as_ref().map(|(id, _)| *id) != Some(resource_id) {
            let result = match &resource.datasource {
                None => Err("no datasource available".to_string()),
                Some(ds) => BmpImporter
                    .import(ds)
                    .map_err(|e| e.to_string())
                    .map(|bmp| {
                        let w = bmp.image.width() as usize;
                        let h = bmp.image.height() as usize;
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [w, h],
                            bmp.image.as_raw(),
                        );
                        ui.ctx().load_texture(
                            format!("bmp_{resource_id}"),
                            color_image,
                            egui::TextureOptions::default(),
                        )
                    }),
            };
            self.cached = Some((resource_id, result));
        }

        let (_, result) = self.cached.as_ref().unwrap();
        match result {
            Err(msg) => {
                ui.centered_and_justified(|ui| {
                    ui.label(format!("Error loading BMP: {msg}"));
                });
            }
            Ok(texture) => {
                let available = ui.available_size();
                let natural = texture.size_vec2();
                let scale = (available.x / natural.x)
                    .min(available.y / natural.y)
                    .min(1.0);
                let display = natural * scale;

                let y_offset = ((available.y - display.y) / 2.0).max(0.0);
                if y_offset > 0.0 {
                    ui.add_space(y_offset);
                }
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    ui.add(egui::Image::new(texture).fit_to_exact_size(display));
                });
            }
        }
    }
}
