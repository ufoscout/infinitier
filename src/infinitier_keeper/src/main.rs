mod app;
mod save;
mod state;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use eframe::egui;
use infinitier_core::{fs::CaseInsensitiveFS, game::GameDataBuilder, game_detect::detect_game};
use infinitier_datasource::{DataSource, Importer};
use infinitier_tlk_resource::{Tlk, TlkImporter};

use crate::app::KeeperApp;
use crate::state::AppState;

/// Infinitier Keeper — cross-engine Infinity Engine save-game editor.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Comma-separated list of game folders (must contain a `CHITIN.KEY`
    /// file in at least the first one). Used to detect which engine
    /// produced the save and to look up shared game data (names, 2DA
    /// tables, …) — none of this is hard-coded by the keeper. When
    /// multiple folders are given they are merged into a single
    /// case-insensitive view in input order — later folders override
    /// earlier ones on path conflicts (mod-overlay style).
    #[arg(long, value_delimiter = ',', required = true, num_args = 1..)]
    game_path: Vec<PathBuf>,
    /// Path to a single save folder — e.g.
    /// `<game>/save/000000001-Quick-Save/`. Must contain exactly one
    /// `.GAM` file.
    #[arg(long)]
    save_path: PathBuf,
    /// Optional path to `dialog.tlk` — used to resolve localized
    /// party-member names (Minsc, Imoen, Aerie, …) referenced by
    /// CRE strrefs. When omitted we auto-detect against the game
    /// folder; see [`locate_dialog_tlk`] for the search order.
    #[arg(long)]
    tlk_path: Option<PathBuf>,
    /// Log filter, e.g. "warn", "debug", "infinitier=debug,warn".
    #[arg(long, default_value = "infinitier=debug,warn")]
    log: String,
}

/// Look for a `dialog.tlk` to use for strref resolution. Searches each
/// supplied `game_path` in input order; for every folder it tries (in
/// order):
///
/// 1. `<game_path>/lang/en_us/dialog.tlk` — the canonical EE layout
///    with English. We prefer English on auto-detect because the
///    keeper's typed-field labels (Strength, …) are English; the
///    user can pass `--tlk-path` to override.
/// 2. Any `<game_path>/lang/<locale>/dialog.tlk` — first one found.
/// 3. `<game_path>/dialog.tlk` — older non-EE layout.
fn locate_dialog_tlk(game_paths: &[PathBuf]) -> Option<PathBuf> {
    for game_path in game_paths {
        let lang_root = game_path.join("lang");
        for preferred in ["en_us", "en_US", "en"] {
            let p = lang_root.join(preferred).join("dialog.tlk");
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(entries) = std::fs::read_dir(&lang_root) {
            for entry in entries.flatten() {
                let p = entry.path().join("dialog.tlk");
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        let direct = game_path.join("dialog.tlk");
        if direct.is_file() {
            return Some(direct);
        }
    }
    None
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

fn load_tlk(path: &std::path::Path) -> std::io::Result<Tlk> {
    TlkImporter {
        name: path.to_str().unwrap_or("dialog.tlk"),
    }
    .import(&DataSource::new(path))
}

fn main() {
    let args = Args::parse();
    env_logger::Builder::new().parse_filters(&args.log).init();

    // Game detection drives every engine-specific decision below — we
    // never read it from CLI flags or a config file.
    let game = detect_game(
        &CaseInsensitiveFS::new(args.game_path.as_slice()).unwrap_or_else(|e| {
            eprintln!(
                "Failed to open game folder(s) [{}]: {e}",
                display_paths(&args.game_path),
            );
            std::process::exit(1);
        }),
    )
    .unwrap_or_else(|| {
        eprintln!(
            "Cannot detect game type at [{}]",
            display_paths(&args.game_path),
        );
        std::process::exit(1);
    });

    let game_data = GameDataBuilder::new(args.game_path.as_slice(), game)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
            eprintln!(
                "Failed to load game data from [{}]: {e}",
                display_paths(&args.game_path),
            );
            std::process::exit(1);
        });

    // Locate dialog.tlk so we can render party-member names instead
    // of the 8-byte engine script-names. Failures are non-fatal —
    // we just fall back to the GAM long-name / script-name chain.
    let tlk_path = args
        .tlk_path
        .clone()
        .or_else(|| locate_dialog_tlk(&args.game_path));
    let tlk = match &tlk_path {
        Some(p) => match load_tlk(p) {
            Ok(t) => {
                log::info!(
                    "Loaded TLK '{}': lang_id={} entries={}",
                    p.display(),
                    t.language_id,
                    t.len()
                );
                Some(t)
            }
            Err(e) => {
                log::warn!("Failed to load TLK '{}': {e}", p.display());
                None
            }
        },
        None => {
            log::warn!(
                "No dialog.tlk found under [{}]/lang — party-member names will fall back to engine script-names.",
                display_paths(&args.game_path),
            );
            None
        }
    };

    let save = save::load_save(&args.save_path, game.engine(), tlk.as_ref()).unwrap_or_else(|e| {
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
        eprintln!("Failed to run keeper: {e}");
    }
}
