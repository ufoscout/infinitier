//! Builds the left-rail flat tree list from `AppState::groups` and
//! dispatches clicks: a click on a group row flips its expansion and
//! rebuilds the model; a click on a leaf row selects the resource and
//! routes through `viewer::show`.
//!
//! The "flat tree" representation is the same trick as the keeper's
//! party list — a `Vec<TreeNode>` that the Slint `ListView` renders in
//! order. Group rows carry a triangle marker and an extension label;
//! leaf rows carry the resource name.

use std::rc::Rc;

use slint::Model;

use crate::state::AppState;
use crate::ui::{info, viewer};
use crate::{MainWindow, TreeNode};

/// Initial seed: build the tree with every group collapsed and no
/// selection. Called once at startup from `app::run`.
pub fn populate(window: &MainWindow, state: &Rc<AppState>) {
    rebuild_model(window, state);
    window.set_tree_selected(-1);
}

/// Re-derive the flat tree model from the current expansion state and
/// push it onto the window. Called whenever a group toggles.
fn rebuild_model(window: &MainWindow, state: &Rc<AppState>) {
    let expanded = state.group_expanded.borrow();
    let mut rows: Vec<TreeNode> = Vec::new();
    for (gi, &ext) in state.group_order.iter().enumerate() {
        let entries = &state.groups[ext];
        rows.push(TreeNode {
            label: format!("{} ({})", ext, entries.len()).into(),
            is_group: true,
            expanded: expanded[gi],
            id: gi as i32,
        });
        if expanded[gi] {
            for (leaf_label, idx) in entries {
                rows.push(TreeNode {
                    label: leaf_label.clone().into(),
                    is_group: false,
                    expanded: false,
                    id: *idx as i32,
                });
            }
        }
    }
    window.set_tree_nodes(slint::ModelRc::new(slint::VecModel::from(rows)));
}

/// Handle a click on tree-row `idx`.
pub fn on_node_clicked(window: &MainWindow, state: &Rc<AppState>, idx: i32) {
    let Ok(i) = usize::try_from(idx) else { return };
    let model = window.get_tree_nodes();
    let Some(node) = model.row_data(i) else { return };

    if node.is_group {
        let gi = node.id as usize;
        {
            let mut expanded = state.group_expanded.borrow_mut();
            if let Some(slot) = expanded.get_mut(gi) {
                *slot = !*slot;
            }
        }
        rebuild_model(window, state);
        return;
    }

    // Leaf — select the resource and refresh the viewer + info bar.
    let resource_idx = node.id as usize;
    window.set_tree_selected(idx);
    viewer::show(window, state, resource_idx);
    info::show(window, state, resource_idx);
}
