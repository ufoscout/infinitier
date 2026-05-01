mod app;
mod components;
mod state;
mod ui;

use clap::Parser;
use infinitier_core::game::GameDataBuilder;
use std::path::PathBuf;

/// Infinitier Explorer (Iced) — browse resources from Infinity Engine games.
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

fn main() -> iced::Result {
    let args = Args::parse();

    env_logger::Builder::new().parse_filters(&args.log).init();

    let game_data = GameDataBuilder::new(&args.game_path)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
            eprintln!(
                "Failed to load key file from '{}': {e}",
                args.game_path.display()
            );
            std::process::exit(1);
        });

    app::run(game_data)
}
