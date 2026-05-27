//! Image-resource viewer — every raster type that ends up in
//! `ImportedResource::Image` (BMP / PVRZ / MOS / PNG).

use infinitier_core::game::GameResource;
use infinitier_core::imported_resource::image::ImportedImage;
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

use crate::MainWindow;
use crate::ui::viewers::common;

pub fn populate(window: &MainWindow, img: ImportedImage, resource: &GameResource) {
    let w = img.width();
    let h = img.height();
    let format_label = img.format_label();
    let format_description = img.format_description();

    let buffer =
        SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(img.image.as_raw(), w, h);
    let image = Image::from_rgba8(buffer);

    window.set_viewer_kind("image".into());
    window.set_image_bitmap(image);
    window.set_image_dims(format!("{w} × {h} px").into());
    window.set_image_file_size(common::file_size_text(resource).into());
    window.set_image_format_label(format_label.into());
    window.set_image_format_description(format_description.into());
    window.set_image_origin(common::origin_text(resource).into());
}
