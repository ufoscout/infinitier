//! GPUI port of infinitier_keeper.

mod app;
mod components;
mod load;
mod state;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use gpui::{AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

use crate::app::KeeperApp;

/// Infinitier Keeper (GPUI) — cross-engine Infinity Engine save-game editor.
#[derive(Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Comma-separated list of game folders (at least one must contain
    /// a `CHITIN.KEY`).
    #[arg(long, value_delimiter = ',', required = true, num_args = 1..)]
    pub game_path: Vec<PathBuf>,
    /// Save game to open — numeric index (alphabetical, 0-based) or
    /// on-disk save folder name.
    #[arg(long)]
    pub savegame: String,
    /// Log filter, e.g. "warn", "debug", "infinitier=debug,warn".
    #[arg(long, default_value = "infinitier=debug,warn")]
    pub log: String,
}

fn main() {
    let args = Args::parse();
    env_logger::Builder::new().parse_filters(&args.log).init();

    let state = match load::load(&args) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Startup failed: {e}");
            std::process::exit(1);
        }
    };

    Application::new().run(move |cx| {
        gpui_component::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Default::default(),
                size: size(px(1400.), px(800.)),
            })),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let view = cx.new(|_| KeeperApp::new(state));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("open window");
    });
}
