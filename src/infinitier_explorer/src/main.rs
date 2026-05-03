mod app;
mod components;
mod state;
mod ui;

use clap::Parser;
use eframe::egui;
use infinitier_core::game::GameDataBuilder;
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

    let key = GameDataBuilder::new(&args.game_path)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
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
            // On Linux/X11, winit reads xrandr physical dimensions (~189 DPI on HiDPI
            // laptops) rather than the X server's pre-configured DPI (~96 DPI), causing
            // pixels_per_point to be set ~2x too high. Honor INFINITIER_SCALE if set,
            // otherwise cap at 1.5 to prevent runaway zoom on misconfigured displays.
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
            Ok(Box::new(app::ExplorerApp::new(key)))
        }),
    ) {
        eprintln!("Failed to run explorer: {e}");
    }
}
