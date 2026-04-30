mod app;
mod components;
mod ui;

use eframe::egui;
use infinitier_datasource::{DataSource, Importer};
use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};
use infinitier_key_importer::{Key, KeyImporter};
use std::path::PathBuf;

fn main() {
    let game_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!(
                "Usage: infinitier_explorer <game_folder>\n\
                 Supported games: bg, bg2, bgee, bg2ee, idw, idwee, idw2, pst, pstee"
            );
            std::process::exit(1);
        });

    let key = load_key(&game_path).unwrap_or_else(|e| {
        eprintln!("Failed to load key file from '{}': {e}", game_path.display());
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
