//! GPUI port of infinitier_explorer.

mod app;
mod components;
mod load;
mod state;
mod ui;

use std::path::PathBuf;

use clap::Parser;
use gpui::{AppContext, Application, Bounds, KeyBinding, WindowBounds, WindowOptions, px, size};
use gpui_component::Root;

use crate::app::ExplorerApp;
use crate::components::key_file_tree_view::{
    KEY_FILE_TREE_CONTEXT, SelectDown, SelectLeft, SelectRight, SelectUp,
};

/// Infinitier Explorer (GPUI) — browse resources from Infinity Engine games.
#[derive(Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Comma-separated list of game folders (bg, bg2, bgee, bg2ee, idw, idwee,
    /// idw2, pst, pstee). At least the first folder must contain a CHITIN.KEY
    /// file. When multiple folders are given they are merged into a single
    /// case-insensitive view in input order — later folders override earlier
    /// ones on path conflicts (mod-overlay style).
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

    Application::new().run(move |cx| {
        gpui_component::init(cx);


        // Bind arrow keys to the existing gpui-component navigation
        // actions, scoped to the resource-tree key context. The tree
        // wrapper claims focus + dispatches on these actions.
        cx.bind_keys([
            KeyBinding::new("up", SelectUp, Some(KEY_FILE_TREE_CONTEXT)),
            KeyBinding::new("down", SelectDown, Some(KEY_FILE_TREE_CONTEXT)),
            KeyBinding::new("left", SelectLeft, Some(KEY_FILE_TREE_CONTEXT)),
            KeyBinding::new("right", SelectRight, Some(KEY_FILE_TREE_CONTEXT)),
        ]);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Default::default(),
                size: size(px(1400.), px(800.)),
            })),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            let view = cx.new(|cx| ExplorerApp::new(state, cx));
            cx.new(|cx| Root::new(view, window, cx))
        })
        .expect("open window");
    });
}
