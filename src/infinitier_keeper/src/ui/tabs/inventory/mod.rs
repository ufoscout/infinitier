//! The "Inventory" tab: a creature's equipped and carried items.
//!
//! The item slots and item list live in the CRE; each item's name and
//! inventory icon are resolved from its `.itm` file (and `dialog.tlk`),
//! so this tab takes `GameData`. Rows are **selectable**: the chosen slot
//! is the target the Item Browser's "Add to Inventory" writes into. That
//! selection is the tab's own transient UI state — kept in egui's memory
//! (the frame store), not on the global [`AppState`] — and resets when a
//! different creature is shown.

mod view;

use eframe::egui;

use infinitier_core::imported_resource::cre::InventoryRow;
use infinitier_core::imported_resource::gam::NpcCre;
use infinitier_core::resource::cre::Cre;

use super::CharacterTab;
use crate::state::AppState;

/// Frame-store key for the selected slot. A companion `(SLOT_KEY, "anchor")`
/// entry records which creature it belongs to, so the selection clears when
/// a different one is shown.
const SLOT_KEY: &str = "inventory_selected_slot";
/// Frame-store key for a pending "reveal this item in the browser" request,
/// raised by double-clicking a filled slot and consumed by the host.
const BROWSE_KEY: &str = "inventory_browse_request";

pub struct InventoryTab;

impl InventoryTab {
    pub fn show(&self, ui: &mut egui::Ui, state: &mut AppState) {
        // Snapshot the rows from the selected creature (the `cre` borrow ends
        // here), so the selectable render below can take `&mut state`.
        let Some(rows) = selected_inventory(state) else {
            ui.label("Empty party slot — no creature record to edit.");
            return;
        };
        let key = char_key(state);
        let selected = key.and_then(|k| stored_slot(ui.ctx(), k));
        let event = view::render(ui, &rows, &state.game_data, selected);
        if let Some(slot) = event.clicked_slot
            && let Some(k) = key
        {
            store_slot(ui.ctx(), k, slot);
        }
        if let Some(resref) = event.browse {
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new(BROWSE_KEY), resref));
        }
        if let Some((slot, quantities)) = event.quantity_edit {
            with_selected_cre_mut(state, |cre| {
                cre.set_inventory_slot_quantities(slot, quantities)
            });
        }
        if let Some(slot) = event.delete_slot {
            with_selected_cre_mut(state, |cre| cre.clear_inventory_slot(slot));
        }
    }
}

/// Run `edit` against the selected party creature's mutable CRE. No-op when
/// the slot is empty or the creature isn't an embedded record.
fn with_selected_cre_mut(state: &mut AppState, edit: impl FnOnce(&mut Cre)) {
    let active = state.active_mut();
    let Some(idx) = active.selected_party_index else {
        return;
    };
    if let Some(member) = active.save.party_npcs.get_mut(idx)
        && let Some(NpcCre::Cre(imported)) = member.cre.as_mut()
    {
        edit(imported.cre_mut());
    }
}

/// Take (read and clear) a pending request to reveal an item in the Item
/// Browser — raised by double-clicking a filled inventory slot. The host
/// consumes this to open the browser (if closed) and select the item.
pub fn take_browse_request(ctx: &egui::Context) -> Option<String> {
    ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new(BROWSE_KEY)))
}

/// The inventory slot the Item Browser may assign into, or `None`. `Some`
/// only when the Inventory tab is the active sub-tab, the selected party
/// member holds an embedded CRE, and a slot has been clicked for it. Reads
/// the tab's own (frame-store) selection — nothing lives on `AppState`.
pub fn assign_target(ctx: &egui::Context, state: &AppState) -> Option<usize> {
    if state.tabs.is_empty() {
        return None;
    }
    let tab = state.active();
    if tab.selected_tab != CharacterTab::Inventory {
        return None;
    }
    let member = tab.save.party_npcs.get(tab.selected_party_index?)?;
    if !matches!(member.cre, Some(NpcCre::Cre(_))) {
        return None;
    }
    stored_slot(ctx, char_key(state)?)
}

/// The selected party creature's inventory rows, or `None` when the slot is
/// empty / not an embedded CRE.
fn selected_inventory(state: &AppState) -> Option<Vec<InventoryRow>> {
    let active = state.active();
    let idx = active.selected_party_index?;
    let member = active.save.party_npcs.get(idx)?;
    match member.cre.as_ref()? {
        NpcCre::Cre(imported) => Some(imported.inventory(state.game_data.game())),
        NpcCre::Ref(_) => None,
    }
}

/// A key identifying the shown creature (save tab + party slot), so the
/// selection in the frame store is per-creature.
fn char_key(state: &AppState) -> Option<u64> {
    let idx = state.active().selected_party_index?;
    Some(((state.active_tab as u64) << 32) | idx as u64)
}

/// The slot remembered for `char_key`, or `None` when the stored selection
/// belongs to a different creature.
fn stored_slot(ctx: &egui::Context, char_key: u64) -> Option<usize> {
    let anchor = ctx.data_mut(|d| d.get_temp::<u64>(egui::Id::new((SLOT_KEY, "anchor"))));
    (anchor == Some(char_key))
        .then(|| ctx.data_mut(|d| d.get_temp::<usize>(egui::Id::new(SLOT_KEY))))
        .flatten()
}

/// Persist the selected `slot` for `char_key`.
fn store_slot(ctx: &egui::Context, char_key: u64, slot: usize) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new(SLOT_KEY), slot);
        d.insert_temp(egui::Id::new((SLOT_KEY, "anchor")), char_key);
    });
}
