//! Memorization tab — spell-slot table.
//!
//! [`data`] extracts one row per spell-memorisation slot from the CRE
//! (`spell_memorization_info`); [`view`] paints EEKeeper's three-column
//! table (Type · Level · Max Can Memorise) with the "Max Can Memorise"
//! count editable on every game that carries the V1 block (all but IWD2,
//! which hides this tab).

mod data;
mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::Cre;

use crate::state::AppState;

pub struct MemorizationTab;

impl MemorizationTab {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        // Snapshot the rows (the immutable `cre` borrow ends with the
        // block); the editable cells write back via `with_selected_cre_mut`.
        let rows = {
            let Some(cre) = selected_cre(state) else {
                ui.label("Empty party slot — no creature record to edit.");
                return;
            };
            data::memorization_rows(cre)
        };
        view::render(ui, &rows, state);
    }
}

/// The selected party creature, if it's an embedded (parsed) CRE.
fn selected_cre(state: &AppState) -> Option<&Cre> {
    let active = state.active();
    let idx = active.selected_party_index?;
    let member = active.save.party_npcs.get(idx)?;
    match member.cre.as_ref()? {
        NpcCre::Cre(c) => Some(c.cre()),
        NpcCre::Ref(_) => None,
    }
}

/// Run `edit` against the selected party creature's mutable CRE. No-op
/// when the slot is empty or the creature isn't an embedded record.
pub(super) fn with_selected_cre_mut(state: &mut AppState, edit: impl FnOnce(&mut Cre)) {
    let active = state.active_mut();
    let Some(idx) = active.selected_party_index else {
        return;
    };
    let Some(member) = active.save.party_npcs.get_mut(idx) else {
        return;
    };
    if let Some(NpcCre::Cre(c)) = member.cre.as_mut() {
        edit(c.cre_mut());
    }
}
