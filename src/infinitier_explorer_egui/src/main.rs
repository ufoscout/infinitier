mod app;
mod components;
mod state;
mod ui;

use clap::Parser;
use eframe::egui;
use infinitier_core::{fs::CaseInsensitiveFS, game::GameDataBuilder, game_detect::detect_game};
use std::path::PathBuf;

/// Infinitier Explorer — browse resources from Infinity Engine games.
#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    /// Comma-separated list of game folders (bg, bg2, bgee, bg2ee, idw, idwee,
    /// idw2, pst, pstee). At least the first folder must contain a CHITIN.KEY
    /// file. When multiple folders are given they are merged into a single
    /// case-insensitive view in input order — later folders override earlier
    /// ones on path conflicts (mod-overlay style).
    #[arg(value_delimiter = ',', required = true, num_args = 1..)]
    game_path: Vec<PathBuf>,
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

    let game = detect_game(&CaseInsensitiveFS::new(args.game_path.as_slice()).unwrap())
        .expect("Cannot detect game type");

    let key = GameDataBuilder::new(args.game_path.as_slice(), game)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
            eprintln!(
                "Failed to load key file from [{}]: {e}",
                display_paths(&args.game_path),
            );
            std::process::exit(1);
        });

    let title = format!(
        "Infinitier Explorer — {:?} — {}",
        game,
        display_paths(&args.game_path),
    );

    // Force the wgpu GL backend. On Linux the default would be
    // Vulkan, which has shown rendering glitches during window
    // resize on some setups; GL is steadier across drivers.
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::GL;
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(&title)
            .with_clamp_size_to_monitor_size(true)
            .with_maximized(true),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        &title,
        options,
        Box::new(move |cc| {
            infinitier_egui_common::theme::apply(&cc.egui_ctx, &infinitier_egui_common::theme::DARK);

            // On Linux/X11, winit reads xrandr physical dimensions (~189 DPI on HiDPI
            // laptops) rather than the X server's pre-configured DPI (~96 DPI), causing
            // pixels_per_point to be set ~2x too high. Honor INFINITIER_SCALE if set,
            // otherwise cap at 1.5 to prevent runaway zoom on misconfigured displays.
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

            Ok(Box::new(app::ExplorerApp::new(key)))
        }),
    ) {
        eprintln!("Failed to run explorer: {e}");
    }
}
