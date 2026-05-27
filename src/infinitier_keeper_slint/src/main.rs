//! Slint Palette spike of infinitier_keeper.
//!
//! Same code shape as `infinitier_keeper_slint` but the `.slint`
//! files reference the built-in `Palette` global (light/dark
//! auto-switch) instead of a custom `Theme`. The `--color-scheme`
//! CLI flag picks the initial scheme; the header's "Toggle theme"
//! button flips between Dark and Light at runtime via
//! `app::toggle_color_scheme`.

mod app;
mod load;
mod state;
mod ui;

use std::path::PathBuf;

use clap::{Parser, ValueEnum};

slint::include_modules!();

#[derive(Parser)]
#[command(author, version, about)]
pub struct Args {
    #[arg(long, value_delimiter = ',', required = true, num_args = 1..)]
    pub game_path: Vec<PathBuf>,
    #[arg(long)]
    pub savegame: String,
    #[arg(long, default_value = "infinitier=debug,warn")]
    pub log: String,
    /// Initial color scheme for the Slint `Palette` global.
    /// `auto` defers to the OS preference (default); `dark` / `light`
    /// pin one explicitly.
    #[arg(long, value_enum, default_value_t = ColorSchemeArg::Auto)]
    pub color_scheme: ColorSchemeArg,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum ColorSchemeArg {
    Auto,
    Dark,
    Light,
}

impl ColorSchemeArg {
    /// Convert into Slint's `ColorScheme` enum. The slint crate
    /// re-exports it from `slint::language` (and via the generated
    /// `include_modules!()` items too).
    pub fn to_slint(self) -> slint::language::ColorScheme {
        use slint::language::ColorScheme;
        match self {
            ColorSchemeArg::Auto => ColorScheme::Unknown,
            ColorSchemeArg::Dark => ColorScheme::Dark,
            ColorSchemeArg::Light => ColorScheme::Light,
        }
    }
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

    app::run(state, args.color_scheme);
}
