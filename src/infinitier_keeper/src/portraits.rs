//! Cache of decoded party-member portraits.
//!
//! NearInfinity's approach (which we mirror): the BMP shown for a
//! party member comes from the resource named in the CRE header
//! (`large_portrait_*` / `small_portrait_bmp`), resolved against the
//! engine's resource manager. The lookup order is:
//!
//! 1. The game's resource index (`override/` first, then the BIF
//!    archives via `chitin.key`). Standard NPCs ship their
//!    portraits this way (e.g. `XOR1L.BMP` for Xan).
//! 2. The `<game_root>/portraits/` folder, where IE drops custom
//!    portraits when a player imports a `.chr`. The base game
//!    doesn't index this folder so we scan it manually.
//!
//! The `PORTRTn.bmp` files in the save folder are NOT portraits —
//! they're 54×84 thumbnails used by the save-game preview UI.
//!
//! Decoding + RGBA→BGRA conversion is non-trivial enough that we
//! don't want to redo it every frame. Cache key is the lower-cased
//! resref → `Option<Arc<RenderImage>>` (`None` means "tried to load
//! and failed; don't retry"). Cache lives on
//! [`crate::app::KeeperApp`] and is populated lazily during render.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::RenderImage;
use image::Frame;
use infinitier_core::fs::{DataSource, Importer};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::imported_resource::image::ImportedImage;
use infinitier_core::resource::ResourceType;
use infinitier_core::resource::bmp::BmpImporter;
use infinitier_core::resource::cre::Cre;
use smallvec::SmallVec;

use crate::cre_fields;

#[derive(Default)]
pub struct PortraitCache {
    entries: HashMap<String, Option<Arc<RenderImage>>>,
}

impl PortraitCache {
    /// Prefer the CRE's large portrait; fall back to its small
    /// portrait when the large one can't be resolved (e.g. PSTEE,
    /// where the V10 "large portrait" field references a BAM).
    pub fn for_cre(&mut self, cre: &Cre, game_data: &GameData) -> Option<Arc<RenderImage>> {
        let large = &cre_fields::large_portrait_name(cre).to_ascii_lowercase();
        if !large.is_empty()
            && let Some(tex) = self.get_or_load(large, game_data)
        {
            return Some(tex);
        }
        let small = &cre_fields::small_portrait_name(cre).to_ascii_lowercase();
        if small.is_empty() {
            return None;
        }
        self.get_or_load(small, game_data)
    }

    fn get_or_load(&mut self, name: &str, game_data: &GameData) -> Option<Arc<RenderImage>> {
        let key = name.to_ascii_lowercase();
        if let Some(entry) = self.entries.get(&key) {
            return entry.clone();
        }
        let loaded = load_portrait(name, game_data);
        self.entries.insert(key, loaded.clone());
        loaded
    }
}

fn load_portrait(name: &str, game_data: &GameData) -> Option<Arc<RenderImage>> {

    if let Some(resource) = game_data.get_by_name_and_type(name, ResourceType::Bmp)
        && let Ok(ImportedResource::Image(img)) = resource.import(game_data)
    {
        return Some(to_render_image(img));
    }
    // Fallback: `portraits/<name>.bmp` under the game root — IE
    // drops player-imported custom portraits here, and the base
    // resource index does not scan this folder.
    if let Some(path) = find_in_portraits_folder(game_data, name) {
        let ds = DataSource::new(path);
        if let Ok(bmp) = (BmpImporter { name }).import(&ds) {
            return Some(to_render_image(ImportedImage::from_bmp(bmp)));
        }
    }
    None
}

/// Look for `<root>/portraits/<name>.bmp` across every configured
/// game root (case-insensitively).
fn find_in_portraits_folder(game_data: &GameData, name: &str) -> Option<PathBuf> {
    let fs = game_data.fs();
    let needle = format!("portraits/{name}.bmp");
    // `search_path_opt` checks `<root>/<path>` for every root and
    // resolves casing automatically.
    if let Some(found) = fs.search_path_opt(&needle) {
        return Some(found.path().to_path_buf());
    }
    // Some shipped IE engines name it `Portraits/` capitalised on
    // disk and case-sensitive filesystems would skip it without
    // the search_path_opt fallback (which already handles this on
    // every layer the FS knows about) — this manual scan handles
    // installs that drop them somewhere else under each root.
    for root in fs.get_roots() {
        let candidate = Path::new(root).join("portraits").join(format!("{name}.bmp"));
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Convert an RGBA `ImportedImage` into the BGRA `RenderImage` GPUI
/// expects. Same trick the explorer's image viewer uses — swap R↔B
/// in place, wrap in a single-frame `RenderImage`.
fn to_render_image(img: ImportedImage) -> Arc<RenderImage> {
    let buffer = img.image;
    let (width, height) = (buffer.width(), buffer.height());
    let mut raw = buffer.into_raw();
    for pixel in raw.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let frame_buffer = image::ImageBuffer::from_raw(width, height, raw)
        .expect("BGRA buffer keeps the same dimensions as the RGBA source");
    let frame = Frame::new(frame_buffer);
    Arc::new(RenderImage::new(SmallVec::from_elem(frame, 1)))
}
