use super::ResourceViewerTrait;
use bytesize::ByteSize;
use eframe::egui::{self, TextureHandle};
use infinitier_core::{
    game::{DataOrigin, GameResource, ResourceId},
    imported_resource::image::ImportedImage,
};

/// One viewer for every raster image type that lands in
/// [`crate::imported_resource::ImportedResource::Image`] — BMP and PVRZ
/// today, more later. The constructor uploads the RGBA8 buffer to a GPU
/// texture once; subsequent frames only paint.
pub struct ImageViewer {
    cached: TextureHandle,
    /// Short uppercase label of the source format (e.g. `"BMP"`),
    /// rendered as one cell of the info bar.
    format_label: &'static str,
    /// Human-readable detail line (bit depth / compression / DXT
    /// variant), rendered as the next info-bar cell.
    format_description: String,
}

impl ImageViewer {
    pub fn new(img: ImportedImage, ui: &mut egui::Ui, resource_id: ResourceId) -> Self {
        let w = img.width() as usize;
        let h = img.height() as usize;
        let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], img.image.as_raw());
        let cached = ui.ctx().load_texture(
            format!("image_{resource_id}"),
            color_image,
            egui::TextureOptions::default(),
        );

        Self {
            cached,
            format_label: img.format_label(),
            format_description: img.format_description(),
        }
    }
}

impl ResourceViewerTrait for ImageViewer {
    fn show(&mut self, ui: &mut egui::Ui, _resource_id: ResourceId, resource: &GameResource) {
        let texture = &self.cached;

        egui::Panel::bottom("image_info_panel").show_inside(ui, |ui| {
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
                ui.label(self.format_label);
                ui.separator();
                ui.label(&self.format_description);
                ui.separator();
                match &resource.data_origin {
                    DataOrigin::Bif { name } => {
                        ui.label(format!("BIF: {name}"));
                    }
                    DataOrigin::Dir { name, path } => {
                        ui.label(format!("{name}: {}", path.path().display()));
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
