//! Colour-gradient swatches, built from the game's palette BMP.
//!
//! Each appearance colour index selects a gradient ramp — one row of
//! `RANGES12.BMP` (falling back to `MPALETTE.BMP`), which is 12 shades
//! wide × 256 rows tall, brightest shade first. EEKeeper paints each
//! index as a small beveled "pyramid": the brightest shade at the
//! centre stepping down to the darkest at the edges by Chebyshev
//! distance. We reproduce that here so the swatches match.

use eframe::egui;
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::resource::ResourceType;

/// Side length, in pixels, of a rendered swatch.
pub const SWATCH_PX: usize = 24;

/// The palette as up-to-256 gradient ramps, each a list of RGB shades
/// ordered brightest-first.
#[derive(Clone)]
pub struct Palette {
    ramps: Vec<Vec<[u8; 3]>>,
}

impl Palette {
    /// Load the gradient palette BMP. Prefers `RANGES12` (the BG/BG2
    /// 12-shade ramps EEKeeper uses), falling back to `MPALETTE`.
    pub fn load(game_data: &GameData) -> Option<Palette> {
        let img = ["RANGES12", "MPALETTE"].into_iter().find_map(|name| {
            match game_data.import_by_name_and_type(name, ResourceType::Bmp) {
                Ok(Some(ImportedResource::Image(img))) => Some(img),
                _ => None,
            }
        })?;

        let ramps = (0..img.height())
            .map(|y| {
                (0..img.width())
                    .map(|x| {
                        let p = img.image.get_pixel(x, y);
                        [p[0], p[1], p[2]]
                    })
                    .collect()
            })
            .collect();
        Some(Palette { ramps })
    }

    /// Build a swatch image for a colour index: a centre-bright,
    /// edge-dark bevel that walks the ramp's shades outward by Chebyshev
    /// distance. An unknown index renders as a flat black square.
    pub fn swatch(&self, index: u8) -> egui::ColorImage {
        let ramp = self.ramps.get(index as usize).filter(|r| !r.is_empty());
        let n = SWATCH_PX;
        let center = (n - 1) as f32 / 2.0;
        let mut pixels = vec![0u8; n * n * 4];

        for y in 0..n {
            for x in 0..n {
                let d = (x as f32 - center).abs().max((y as f32 - center).abs());
                let rgb = match ramp {
                    Some(shades) => {
                        let t = if center > 0.0 { d / center } else { 0.0 };
                        let idx = ((t * shades.len() as f32) as usize).min(shades.len() - 1);
                        shades[idx]
                    }
                    None => [0, 0, 0],
                };
                let i = (y * n + x) * 4;
                pixels[i..i + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        egui::ColorImage::from_rgba_unmultiplied([n, n], &pixels)
    }
}
