mod app;
mod components;
mod ui;

use clap::Parser;
use eframe::egui;
use infinitier_datasource::{DataSource, Importer};
use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};
use infinitier_key_importer::{Key, KeyImporter};
use std::path::PathBuf;

/// Infinitier Explorer — browse resources from Infinity Engine games.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to the game folder (bg, bg2, bgee, bg2ee, idw, idwee, idw2, pst, pstee).
    /// The folder must contain a CHITIN.KEY file.
    game_path: PathBuf,
    /// Log filter, e.g. "warn", "debug", "infinitier=debug,warn".
    #[arg(long, default_value = "infinitier=debug,warn")]
    log: String,
}

fn main() {
    let args = Args::parse();

    env_logger::Builder::new().parse_filters(&args.log).init();

    let key = load_key(&args.game_path).unwrap_or_else(|e| {
        eprintln!(
            "Failed to load key file from '{}': {e}",
            args.game_path.display()
        );
        std::process::exit(1);
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Infinitier Explorer")
            .with_clamp_size_to_monitor_size(true)
            .with_maximized(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Infinitier Explorer",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::light());
            Ok(Box::new(app::ExplorerApp::new(key)))
        }),
    ) {
        eprintln!("Failed to run explorer: {e}");
    }
}

fn load_key(game_path: &std::path::Path) -> std::io::Result<Key> {
    let fs = CaseInsensitiveFS::new(game_path)?;
    let key_path = fs.get_path(&CaseInsensitivePath::new("CHITIN.KEY"))?;
    KeyImporter::import(&DataSource::new(key_path.as_path()))
}
