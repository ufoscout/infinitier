//! Bootstrap: CLI → GameData → save discovery → TLK → ImportedGam.
//!
//! Mirrors the egui keeper's `main.rs` boot sequence so behaviour is
//! identical; the only Slint-specific change is that the result is
//! stashed in [`AppState`] instead of an `eframe` `AppState`.

use infinitier_core::fs::{CaseInsensitiveFS, Importer};
use infinitier_core::game::GameDataBuilder;
use infinitier_core::game_detect::detect_game;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GamImporter;

use crate::Args;
use crate::state::AppState;

pub fn load(args: &Args) -> std::io::Result<AppState> {
    let game = detect_game(&CaseInsensitiveFS::new(args.game_path.as_slice())?)
        .ok_or_else(|| std::io::Error::other("could not detect game type"))?;

    let game_data = GameDataBuilder::new(args.game_path.as_slice(), game)?.build()?;

    let save_games = game_data.save_games();
    let core_save = if let Ok(idx) = args.savegame.parse::<usize>() {
        save_games.by_index(idx).cloned().ok_or_else(|| {
            std::io::Error::other(format!(
                "savegame index {idx} out of range — {} discovered",
                save_games.len(),
            ))
        })?
    } else {
        save_games
            .by_name(&args.savegame)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("savegame '{}' not found", args.savegame)))?
    };

    let tlk = match game_data.dialog_tlk() {
        Ok(t) => Some(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("No dialog.tlk found; falling back to engine script-names.");
            None
        }
        Err(e) => {
            log::warn!("Failed to load dialog.tlk: {e}");
            None
        }
    };

    let gam = GamImporter {
        name: &core_save.name,
        engine: game.engine(),
    }
    .import(&core_save.gam)?;
    let imported_gam = ImportedGam::load_with_tlk(gam, tlk.as_ref())?;

    Ok(AppState {
        game_data,
        save_name: core_save.name,
        imported_gam,
    })
}
