//! Resistances tab — a creature's combat resistances, saving throws
//! and per-damage-type armor-class modifiers.
//!
//! [`data`] pulls the values out of the CRE header; [`view`] paints
//! the group boxes. On AD&D creatures (V1.0 / V1.2 / V9.0 — including
//! the Enhanced Editions) the fields are **editable**; IWD2 (V2.2) is
//! shown read-only.

mod data;
mod view;

use eframe::egui;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::Cre;

use crate::state::AppState;

pub struct ResistancesTab;

impl ResistancesTab {
    /// Takes `&mut AppState` so the AD&D fields can commit edits.
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        // Snapshot the values for display (the `cre` borrow ends with the
        // block), then render — the editable rows write back via
        // `with_selected_cre_mut`.
        let data = {
            let Some(cre) = selected_cre(state) else {
                ui.label("Empty party slot — no creature record to edit.");
                return;
            };
            data::resist_data(cre)
        };
        view::render(ui, &data, state);
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
