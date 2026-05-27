//! BCS viewer — decompiled BAF source + summary info bar.
//!
//! Slint can't easily reproduce egui's per-token recoloured
//! `TextEdit::layouter`, so the BAF source lands in the Slint pane as
//! plain monospaced text. The CR / line counts in the info bar match
//! the egui original.

use infinitier_core::game::GameResource;
use infinitier_core::imported_resource::bcs::ImportedBcs;

use crate::MainWindow;
use crate::ui::viewers::common;

pub fn populate(window: &MainWindow, bcs: ImportedBcs, resource: &GameResource) {
    let cr_count = bcs.bcs.condition_responses.len();
    let baf_lines = bcs.baf.lines().count();

    window.set_viewer_kind("bcs".into());
    window.set_bcs_baf(bcs.baf.into());
    window.set_bcs_file_size(common::file_size_text(resource).into());
    window.set_bcs_cr_blocks(format!("{cr_count} CR blocks").into());
    window.set_bcs_baf_lines(format!("{baf_lines} BAF lines").into());
    window.set_bcs_origin(common::origin_text(resource).into());
}
