//! Cache of decoded party-member portraits.
//!
//! Mirrors NearInfinity (and the GPUI keeper's `PortraitCache`): the
//! BMP shown for a party member comes from the resource named in the
//! CRE header. Lookup order:
//!
//! 1. The game's resource index (`override/` first, then BIFs).
//! 2. `<game_root>/portraits/<name>.bmp` for player-imported
//!    custom portraits — the base resource index doesn't scan it.
//!
//! egui's renderer takes [`TextureHandle`]s instead of raw image
//! data, so the cached payload is an `Option<TextureHandle>`
//! (`None` is a sticky failure — we don't re-try a missing lookup on
//! every frame).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;
use infinitier_core::fs::{DataSource, Importer};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::imported_resource::image::ImportedImage;
use infinitier_core::resource::ResourceType;
use infinitier_core::resource::bmp::BmpImporter;
use infinitier_core::resource::cre::Cre;

#[derive(Default)]
pub struct PortraitCache {
    entries: HashMap<String, Option<egui::TextureHandle>>,
}

impl PortraitCache {
    /// Prefer the CRE's large portrait; fall back to its small one
    /// when the large portrait can't be resolved (PSTEE's V1.0 ships
    /// a BAM there, not a BMP).
    pub fn for_cre(
        &mut self,
        cre: &Cre,
        game_data: &GameData,
        ctx: &egui::Context,
    ) -> Option<egui::TextureHandle> {
        let large = cre.large_portrait_name().to_ascii_lowercase();
        if !large.is_empty()
            && let Some(tex) = self.get_or_load(&large, game_data, ctx)
        {
            return Some(tex);
        }
        let small = cre.small_portrait_name().to_ascii_lowercase();
        if small.is_empty() {
            return None;
        }
        self.get_or_load(&small, game_data, ctx)
    }

    fn get_or_load(
        &mut self,
        name: &str,
        game_data: &GameData,
        ctx: &egui::Context,
    ) -> Option<egui::TextureHandle> {
        if let Some(entry) = self.entries.get(name) {
            return entry.clone();
        }
        let loaded = load_portrait(name, game_data, ctx);
        self.entries.insert(name.to_string(), loaded.clone());
        loaded
    }
}

fn load_portrait(
    name: &str,
    game_data: &GameData,
    ctx: &egui::Context,
) -> Option<egui::TextureHandle> {
    // `chitin.key` indexes resrefs lowercased; we pass `name` in
    // already-lowercased form so the index hits regardless of the
    // CRE field's original casing.
    if let Some(resource) = game_data.get_by_name_and_type(name, ResourceType::Bmp)
        && let Ok(imported) = resource.import(game_data)
        && let ImportedResource::Image(img) = imported.as_ref()
    {
        return Some(to_texture(ctx, name, img));
    }
    if let Some(path) = find_in_portraits_folder(game_data, name) {
        let ds = DataSource::new(path);
        if let Ok(bmp) = (BmpImporter { name }).import(&ds) {
            return Some(to_texture(ctx, name, &ImportedImage::from_bmp(bmp)));
        }
    }
    None
}

/// Search `<root>/portraits/<name>.bmp` across every configured game
/// root. The first hit wins.
fn find_in_portraits_folder(game_data: &GameData, name: &str) -> Option<PathBuf> {
    let fs = game_data.fs();
    let needle = format!("portraits/{name}.bmp");
    if let Some(found) = fs.search_path_opt(&needle) {
        return Some(found.path().to_path_buf());
    }
    for root in fs.get_roots() {
        let candidate = Path::new(root)
            .join("portraits")
            .join(format!("{name}.bmp"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Upload the decoded RGBA buffer to a GPU texture managed by egui.
/// `egui::ColorImage` expects unmultiplied RGBA, which is exactly
/// what `ImportedImage` carries — no channel swap needed (unlike the
/// gpui port, which has to convert to BGRA).
fn to_texture(ctx: &egui::Context, name: &str, img: &ImportedImage) -> egui::TextureHandle {
    let buffer = &img.image;
    let size = [buffer.width() as usize, buffer.height() as usize];
    let pixels = buffer.as_raw();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels);
    ctx.load_texture(
        format!("party-portrait/{name}"),
        color_image,
        egui::TextureOptions::LINEAR,
    )
}
