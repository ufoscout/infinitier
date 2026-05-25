mod app;
mod cre;
mod save;
mod state;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use eframe::egui;
use infinitier_core::{fs::CaseInsensitiveFS, game::GameDataBuilder, game_detect::detect_game};

use crate::app::KeeperApp;
use crate::state::AppState;

/// Infinitier Keeper — cross-engine Infinity Engine save-game editor.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to the game folder (must contain a `CHITIN.KEY` file).
    /// Used to detect which engine produced the save and to look up
    /// shared game data (names, 2DA tables, …) — none of this is
    /// hard-coded by the keeper.
    #[arg(long)]
    game_path: PathBuf,
    /// Path to a single save folder — e.g.
    /// `<game>/save/000000001-Quick-Save/`. Must contain exactly one
    /// `.GAM` file.
    #[arg(long)]
    save_path: PathBuf,
    /// Log filter, e.g. "warn", "debug", "infinitier=debug,warn".
    #[arg(long, default_value = "infinitier=debug,warn")]
    log: String,
}

fn main() {
    let args = Args::parse();
    env_logger::Builder::new().parse_filters(&args.log).init();

    // Game detection drives every engine-specific decision below — we
    // never read it from CLI flags or a config file.
    let game = detect_game(&CaseInsensitiveFS::new(&args.game_path).unwrap_or_else(|e| {
        eprintln!(
            "Failed to open game folder '{}': {e}",
            args.game_path.display()
        );
        std::process::exit(1);
    }))
    .unwrap_or_else(|| {
        eprintln!(
            "Cannot detect game type at '{}'",
            args.game_path.display()
        );
        std::process::exit(1);
    });

    let game_data = GameDataBuilder::new(&args.game_path, game)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
            eprintln!(
                "Failed to load game data from '{}': {e}",
                args.game_path.display()
            );
            std::process::exit(1);
        });

    let save = save::load_save(&args.save_path, game.engine()).unwrap_or_else(|e| {
        eprintln!(
            "Failed to load save folder '{}': {e}",
            args.save_path.display()
        );
        std::process::exit(1);
    });

    let state = AppState::new(game, args.game_path.clone(), game_data, save);

    let title = format!(
        "Infinitier Keeper — {:?} — {}",
        state.game,
        state.save.save_path.display()
    );

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
                let is_x11 = std::env::var("WAYLAND_DISPLAY").is_err()
                    && std::env::var("DISPLAY").is_ok();
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
        eprintln!("Failed to run keeper: {e}");
    }
}
