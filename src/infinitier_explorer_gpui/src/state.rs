//! Root-view state — game data + current selection. Mirrors the egui
//! `AppState` one-for-one; mutated through `cx.listener` closures when
//! the user clicks a leaf in the resource tree or presses arrow keys.

use std::collections::HashSet;

use infinitier_core::game::{GameData, ResourceId};

use crate::components::key_file_tree_view::FocusedRow;

pub struct AppState {
    pub game_data: GameData,
    /// Which resource the central viewer is showing.
    pub selected_resource: Option<ResourceId>,
    /// Extension groups currently expanded in the left-panel tree.
    /// Persisted on the root view rather than in the tree component
    /// itself so the collapsed/open state survives re-renders.
    pub expanded_groups: HashSet<&'static str>,
    /// Where the tree's keyboard cursor currently lives. Distinct from
    /// `selected_resource` because the cursor can sit on a header
    /// (group label) where there's no resource to select. When the
    /// cursor lands on a leaf via arrow keys we mirror it into
    /// `selected_resource`.
    pub focused_row: Option<FocusedRow>,
}

impl AppState {
    pub fn new(game_data: GameData) -> Self {
        Self {
            game_data,
            selected_resource: None,
            expanded_groups: HashSet::new(),
            focused_row: None,
        }
    }
}
