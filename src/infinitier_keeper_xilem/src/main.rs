//! Xilem port of the Infinitier keeper (read-only). The game-loading
//! path is identical to the egui keeper; only the UI shell differs —
//! instead of `eframe::run_native` we hand the state to Xilem's runner
//! and let `ui::app_logic` rebuild the view tree.

mod cre_fields;
mod fields;
mod state;
mod tabs;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use infinitier_core::engine_caps::EngineCaps;
use infinitier_core::fs::{CaseInsensitiveFS, Importer};
use infinitier_core::game::GameDataBuilder;
use infinitier_core::game_detect::detect_game;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GamImporter;
use xilem::winit::dpi::LogicalSize;
use xilem::winit::error::EventLoopError;
use xilem::{EventLoop, WindowOptions, Xilem};

use crate::state::AppState;

/// Infinitier Keeper (Xilem) — read-only save-game viewer.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Comma-separated list of game folders (one must contain `CHITIN.KEY`).
    #[arg(long, value_delimiter = ',', required = true, num_args = 1..)]
    game_path: Vec<PathBuf>,
    /// Which save game to open: a numeric index or the save folder name.
    #[arg(long)]
    savegame: String,
    /// Log filter, e.g. "warn", "infinitier=debug,warn".
    #[arg(long, default_value = "infinitier=debug,warn")]
    log: String,
}

fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn main() -> Result<(), EventLoopError> {
    let args = Args::parse();
    env_logger::Builder::new().parse_filters(&args.log).init();

    let game = detect_game(
        &CaseInsensitiveFS::new(args.game_path.as_slice()).unwrap_or_else(|e| {
            log::error!(
                "Failed to open game folder(s) [{}]: {e}",
                display_paths(&args.game_path),
            );
            std::process::exit(1);
        }),
    )
    .unwrap_or_else(|| {
        log::error!("Cannot detect game type at [{}]", display_paths(&args.game_path));
        std::process::exit(1);
    });

    let game_data = GameDataBuilder::new(args.game_path.as_slice(), game)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
            log::error!(
                "Failed to load game data from [{}]: {e}",
                display_paths(&args.game_path),
            );
            std::process::exit(1);
        });

    let save_games = game_data.save_games();
    let core_save = if let Ok(idx) = args.savegame.parse::<usize>() {
        save_games.by_index(idx).cloned().unwrap_or_else(|| {
            log::error!(
                "savegame index {idx} out of range — {} save(s) discovered",
                save_games.len(),
            );
            std::process::exit(1);
        })
    } else {
        save_games.by_name(&args.savegame).cloned().unwrap_or_else(|| {
            log::error!("savegame '{}' not found", args.savegame);
            std::process::exit(1);
        })
    };

    let gam = GamImporter {
        name: &core_save.name,
        engine: game.engine(),
    }
    .import(&core_save.gam)
    .unwrap_or_else(|e| {
        eprintln!("Failed to import GAM for '{}': {e}", core_save.name);
        std::process::exit(1);
    });
    let imported_gam = ImportedGam::load(gam, &game_data).unwrap_or_else(|e| {
        eprintln!("Failed to resolve save '{}': {e}", core_save.name);
        std::process::exit(1);
    });

    let engine_caps = EngineCaps::new(&game_data).unwrap_or_else(|e| {
        log::error!("Failed to build EngineCaps: {e}");
        std::process::exit(1);
    });

    let state = AppState::new(
        game_data,
        core_save.name,
        core_save.folder_path,
        Box::new(imported_gam),
        engine_caps,
    );
    let title = state.window_title();

    let app = Xilem::new_simple(
        state,
        ui::app_logic,
        WindowOptions::new(title).with_min_inner_size(LogicalSize::new(900.0, 540.0)),
    );
    app.run_in(EventLoop::with_user_event())
}
