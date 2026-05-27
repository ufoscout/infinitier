//! Slint port of infinitier_explorer.

mod app;
mod load;
mod state;
mod ui;

use std::path::PathBuf;

use clap::Parser;

slint::include_modules!();

/// Infinitier Explorer (Slint) — browse resources from Infinity Engine games.
#[derive(Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Comma-separated list of game folders. At least the first folder
    /// must contain a `CHITIN.KEY` file. When multiple folders are given
    /// they are merged into a single case-insensitive view in input
    /// order — later folders override earlier ones on path conflicts
    /// (mod-overlay style).
    #[arg(value_delimiter = ',', required = true, num_args = 1..)]
    pub game_path: Vec<PathBuf>,
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

    app::run(state);
}
