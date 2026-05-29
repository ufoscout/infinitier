//! Bootstrap: CLI → GameData → save discovery → TLK → ImportedGam.
//!
//! Identical sequence to `infinitier_keeper_slint::load`; only the
//! return type differs (raw `KeeperState`, no `Rc` wrapping — the GPUI
//! entity ends up owning it directly).

use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::fs::{CaseInsensitiveFS, Importer};
use infinitier_core::game::GameDataBuilder;
use infinitier_core::game_detect::detect_game;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GamImporter;

use crate::Args;
use crate::state::KeeperState;

pub fn load(args: &Args) -> std::io::Result<KeeperState> {
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
        save_games.by_name(&args.savegame).cloned().ok_or_else(|| {
            std::io::Error::other(format!("savegame '{}' not found", args.savegame))
        })?
    };

    let gam = GamImporter {
        name: &core_save.name,
        engine: game.engine(),
    }
    .import(&core_save.gam)?;
    let imported_gam = ImportedGam::load(gam, &game_data)?;
    let engine_caps = EngineCaps::new(&game_data)?;

    Ok(KeeperState {
        game_data,
        save_name: core_save.name,
        save_folder_path: core_save.folder_path,
        imported_gam,
        engine_caps,
    })
}
