//! Read-only extraction for the Inventory tab.
//!
//! A creature carries a flat list of items plus an `item_slots` lookup
//! table whose position encodes the equipment slot (helmet, armor, the
//! four quick-weapon slots, quivers, inventory, …) and whose value is an
//! index into that item list (a negative index meaning "empty"). This
//! module flattens the two into one ordered row per displayed slot,
//! mirroring EEKeeper's "Position / Quantity / Item / Resource" table.

use infinitier_core::resource::cre::{Cre, Item, SubSections};

/// One row of the inventory table: a slot and the item (if any) in it.
pub struct InventoryRow {
    /// Slot name shown in the "Position" column.
    pub position: &'static str,
    /// `charges1/charges2/charges3` for a filled slot; empty otherwise.
    pub quantity: String,
    /// The item's `.itm` resref for a filled slot; empty otherwise.
    pub resref: String,
}

/// The BG / BG2 / IWD (CRE V1.0 / V9.0) item-slot layout — the ordering
/// EEKeeper displays. The on-disk array has 40 entries; the trailing two
/// (the selected-weapon and selected-ability indices) are not real item
/// slots, so they're omitted here, exactly as EEKeeper does.
const BG_SLOT_LABELS: &[&str] = &[
    "Helmet",
    "Armor",
    "Shield",
    "Gauntlets",
    "Ring",
    "Ring",
    "Amulet",
    "Belt",
    "Boots",
    "Quick Weapon",
    "Quick Weapon",
    "Quick Weapon",
    "Quick Weapon",
    "Quiver",
    "Quiver",
    "Quiver",
    "Quiver",
    "Cloak",
    "Quick Item",
    "Quick Item",
    "Quick Item",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Inventory",
    "Magic Weapon",
];

/// Build one display row per slot, in EEKeeper's order.
pub fn inventory_rows(cre: &Cre) -> Vec<InventoryRow> {
    let (slots, items) = slots_and_items(cre);
    let labels = slot_labels(slots.len());

    labels
        .iter()
        .enumerate()
        .map(|(i, &position)| {
            let item = slots
                .get(i)
                .copied()
                .filter(|&idx| idx >= 0)
                .and_then(|idx| items.get(idx as usize));
            match item {
                Some(it) => InventoryRow {
                    position,
                    quantity: format!("{}/{}/{}", it.quantity1, it.quantity2, it.quantity3),
                    resref: it.item.clone(),
                },
                None => InventoryRow {
                    position,
                    quantity: String::new(),
                    resref: String::new(),
                },
            }
        })
        .collect()
}

/// Pick the slot-name list for this creature's slot-table size. Only the
/// 40-slot BG family layout is named (the engine this tab targets);
/// other engines fall back to nothing so we simply show no rows rather
/// than mislabel them.
fn slot_labels(slot_count: usize) -> &'static [&'static str] {
    if slot_count == 40 {
        BG_SLOT_LABELS
    } else {
        &[]
    }
}

/// Read the item-slot table (as signed indices) and the item list out of
/// whichever sub-section layout this CRE uses.
fn slots_and_items(cre: &Cre) -> (Vec<i16>, &[Item]) {
    match &cre.sub_sections {
        SubSections::V1(v1) => (as_i16(&v1.item_slots), &v1.items),
        SubSections::V22(v22) => (as_i16(&v22.item_slots), &v22.items),
    }
}

/// The `item_slots` table is stored as little-endian `i16` indices.
fn as_i16(bytes: &[u8]) -> Vec<i16> {
    bytes
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}
