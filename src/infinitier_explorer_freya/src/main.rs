mod app;
mod components;
mod state;
mod ui;

use clap::Parser;
use freya::prelude::*;
use infinitier_core::game::GameDataBuilder;
use std::path::PathBuf;

#[derive(Parser)]
#[command(author, version, about)]
struct Args {
    game_path: PathBuf,
    #[arg(long, default_value = "infinitier=debug,warn")]
    log: String,
}

fn main() {
    let args = Args::parse();
    env_logger::Builder::new().parse_filters(&args.log).init();

    let game_data = GameDataBuilder::new(&args.game_path)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
            eprintln!("Failed to load key file from '{}': {e}", args.game_path.display());
            std::process::exit(1);
        });

    state::INIT_GAME_DATA
        .set(std::sync::Mutex::new(Some(game_data)))
        .ok();

    launch(LaunchConfig::new().with_window(WindowConfig::new(app::app)));
}
