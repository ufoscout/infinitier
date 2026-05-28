//! Root-view state — only the things multiple components actually
//! share. Tree-specific state (expansion, keyboard cursor) lives on
//! `KeyFileTreeView` itself; the central viewer + bottom info bar
//! only need to know which resource is currently selected.

use infinitier_core::game::{GameData, ResourceId};

pub struct AppState {
    pub game_data: GameData,
    /// Which resource the central viewer is showing. Written by the
    /// tree (click + keyboard-cursor-lands-on-leaf), read by the
    /// central panel and the bottom info bar.
    pub selected_resource: Option<ResourceId>,
}

impl AppState {
    pub fn new(game_data: GameData) -> Self {
        Self {
            game_data,
            selected_resource: None,
        }
    }
}
