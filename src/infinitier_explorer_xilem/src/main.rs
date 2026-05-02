mod app;
mod components;
mod state;

use clap::Parser;
use infinitier_core::game::GameDataBuilder;
use std::path::PathBuf;

/// Infinitier Explorer (Xilem) — browse resources from Infinity Engine games.
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

    let game_data = GameDataBuilder::new(&args.game_path)
        .and_then(|b| b.build())
        .unwrap_or_else(|e| {
            eprintln!(
                "Failed to load key file from '{}': {e}",
                args.game_path.display()
            );
            std::process::exit(1);
        });

    let groups = state::build_groups(&game_data);
    let app_state = state::AppState::new(game_data, groups);

    app::run(app_state);
}
