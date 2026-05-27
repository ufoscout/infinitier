mod app;
mod state;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use eframe::egui;
use infinitier_core::fs::{CaseInsensitiveFS, Importer};
use infinitier_core::game::GameDataBuilder;
use infinitier_core::game_detect::detect_game;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GamImporter;

use crate::app::KeeperApp;
use crate::state::AppState;

/// Infinitier Keeper — cross-engine Infinity Engine save-game editor.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Comma-separated list of game folders (must contain a `CHITIN.KEY`
    /// file in at least one). When
    /// multiple folders are given they are merged into a single
    /// case-insensitive view in input order — later folders override
    /// earlier ones on path conflicts (mod-overlay style).
    #[arg(long, value_delimiter = ',', required = true, num_args = 1..)]
    game_path: Vec<PathBuf>,
    /// Which save game to open. Accepts either:
    /// - a numeric index (alphabetical
    ///   by save folder name, starting at 0), or
    /// - the on-disk save folder name (e.g. `000000001-Quick-Save`).
    #[arg(long)]
    savegame: String,
    /// Log filter, e.g. "warn", "debug", "infinitier=debug,warn".
    #[arg(long, default_value = "infinitier=debug,warn")]
    log: String,
}

/// Render a `Vec<PathBuf>` as a comma-separated string for log / window-title
/// purposes.
fn display_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn main() {
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
        log::error!(
            "Cannot detect game type at [{}]",
            display_paths(&args.game_path),
        );
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
        save_games
            .by_name(&args.savegame)
            .cloned()
            .unwrap_or_else(|| {
                log::error!(
                    "savegame '{}' not found — {} save(s) discovered: {}",
                    args.savegame,
                    save_games.len(),
                    save_games
                        .saves()
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                std::process::exit(1);
            })
    };

    // Parse the GAM through its DataSource (from the discovered save
    // folder) then resolve it via `ImportedGam` so every embedded CRE
    // is pre-parsed and every NPC name is TLK-resolved. Strict mode:
    // a corrupt embedded CRE aborts the open with a clear error.
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
        eprintln!("Failed to resolve save '{}': {e}", core_save.name,);
        std::process::exit(1);
    });

    let title = format!("Infinitier Keeper — {:?} — {}", game, core_save.name);

    let state = AppState::new(game_data, core_save.name, Box::new(imported_gam));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_clamp_size_to_monitor_size(true)
            .with_maximized(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());

            // Same Linux/X11 DPI workaround as the explorer crate.
            #[cfg(target_os = "linux")]
            {
                let is_x11 =
                    std::env::var("WAYLAND_DISPLAY").is_err() && std::env::var("DISPLAY").is_ok();
                if is_x11 {
                    if let Ok(scale) = std::env::var("INFINITIER_SCALE") {
                        if let Ok(ppp) = scale.parse::<f32>() {
                            cc.egui_ctx.set_pixels_per_point(ppp);
                        }
                    } else {
                        let ppp = cc.egui_ctx.pixels_per_point();
                        if ppp > 1.5 {
                            cc.egui_ctx.set_pixels_per_point(1.5);
                        }
                    }
                }
            }

            Ok(Box::new(KeeperApp::new(state)))
        }),
    ) {
        log::error!("Failed to run keeper: {e}");
    }
}
