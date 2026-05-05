use super::ResourceViewerTrait;
use bytesize::ByteSize;
use eframe::egui::{self, TextureHandle};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    resource::bmp::{Bmp, BmpCompression},
};

pub struct BmpViewer {
    cached: TextureHandle,
    bit_count: u16,
    compression: BmpCompression,
}

impl BmpViewer {
    pub fn new(bmp: Bmp, ui: &mut egui::Ui, resource_id: ResourceId) -> Self {
        let w = bmp.image.width() as usize;
        let h = bmp.image.height() as usize;
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], bmp.image.as_raw());
        let handle = ui.ctx().load_texture(
            format!("bmp_{resource_id}"),
            color_image,
            egui::TextureOptions::default(),
        );

        Self {
            cached: handle,
            bit_count: bmp.bit_count,
            compression: bmp.compression,
        }
    }
}

impl ResourceViewerTrait for BmpViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        let texture = &self.cached;

        egui::Panel::bottom("bmp_info_panel").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                let [w, h] = texture.size();
                ui.label(format!("{w} × {h} px"));
                ui.separator();
                match resource.file_size {
                    Some(size) => {
                        ui.label(ByteSize(size).to_string());
                    }
                    None => {
                        ui.label("? B");
                    }
                }
                ui.separator();
                ui.label(format!("{} bpp", self.bit_count));
                ui.separator();
                ui.label(self.compression.to_string());
                ui.separator();
                match &resource.data_origin {
                    DataOrigin::Bif { name } => {
                        ui.label(format!("BIF: {name}"));
                    }
                    DataOrigin::Override { path } => {
                        ui.label(format!("Override: {}", path.display()));
                    }
                    DataOrigin::Missing => {
                        ui.label("Missing");
                    }
                }
            });
        });

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
