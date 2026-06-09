//! [`ImportedCre`] — a higher-level, owning wrapper around a [`Cre`].
//!
//! The [`Cre`] resource type stays a pure parser/serialiser of the on-disk
//! format; anything that needs *other* game resources to interpret a CRE
//! field — KITLIST.2DA / HATERACE.2DA name strrefs, IDS symbols, SPL/ITM
//! lookups, … — lives here, where a [`GameData`] (passed per call) resolves
//! them. That keeps the `infinitier_cre_resource` crate free of dependencies
//! on the other resource crates.
//!
//! This is the form stored in [`ImportedResource::Cre`](super::ImportedResource::Cre).

use infinitier_common::Game;
use infinitier_cre_resource::{Cre, Item, SubSections};
use infinitier_two_da_resource::TwoDA;

use crate::game::GameData;

/// One row of the inventory table: an equipment slot and the item (if any)
/// occupying it. The view formats quantity/charges and resolves the item's
/// `.itm` for its name and icon.
#[derive(Debug, Clone)]
pub struct InventoryRow {
    /// Slot name shown in the "Position" column.
    pub position: &'static str,
    /// The carried item in this slot, or `None` when the slot is empty.
    pub item: Option<Item>,
}

/// An owned [`Cre`] plus the higher-level queries that resolve its fields
/// against other resources (2DA / IDS / TLK / …), each taking the
/// [`GameData`] to resolve against.
#[derive(Debug, Clone)]
pub struct ImportedCre {
    cre: Box<Cre>,
}

impl ImportedCre {
    /// Wrap an owned `cre`.
    pub fn new(cre: Cre) -> Self {
        Self { cre: Box::new(cre) }
    }

    /// The wrapped CRE.
    pub fn cre(&self) -> &Cre {
        &self.cre
    }

    /// Mutable access to the wrapped CRE (for edits).
    pub fn cre_mut(&mut self) -> &mut Cre {
        &mut self.cre
    }

    /// Unwrap back into the owned [`Cre`].
    pub fn into_cre(self) -> Cre {
        *self.cre
    }

    /// The `KITLIST.2DA` display-name strref for this creature's kit: the
    /// mixed-case `MIXED` column, falling back to `LOWER`. `None` when the
    /// creature has no kit, `KITLIST.2DA` is unavailable, or no row matches.
    ///
    /// A kitted creature stores the kit as `0x4000 | KITLIST_row_index` in
    /// the high word of the CRE kit dword (so e.g. the Inquisitor at KITLIST
    /// row 5 reads `0x40050000`; the engine uses `kit >> 16`). We mask off the
    /// `0x4000` "has kit" marker to recover the row index and look the row up
    /// by its (numeric) label — which is the index in every game, whereas the
    /// `KIT.IDS` usability flags and the optional `KITIDS` column are not
    /// consistent across BG2 vs the EEs. Resolving the strref to a string
    /// against `dialog.tlk` is the caller's job (the UI owns the tlk cache).
    pub fn kit_strref(&self, game_data: &GameData) -> Option<u32> {
        let kit = self.cre.kit()?;
        let high_word = kit >> 16;
        let kitlist = game_data.import_2da_by_name("kitlist").ok()?;
        kit_strref_from_2da(&kitlist, high_word)
    }

    /// The `HATERACE.2DA` `STRREF` for this creature's racial enemy, matched
    /// on the `IDS` column (the `RACE.IDS` value the byte holds). `None` when
    /// there's no enemy, `HATERACE.2DA` is unavailable, or no row matches.
    pub fn racial_enemy_strref(&self, game_data: &GameData) -> Option<u32> {
        let value = self.cre.racial_enemy()?;
        let haterace = game_data.import_2da_by_name("haterace").ok()?;
        haterace_strref_from_2da(&haterace, value)
    }

    /// The creature's inventory as one [`InventoryRow`] per displayed
    /// equipment slot, in the game's slot order (helmet, weapons, quivers,
    /// inventory, …). `game` selects the slot layout — it differs across the
    /// BG/BG2/IWD family, classic PST, and PST:EE. Empty slots yield a row
    /// with `item == None`; an unsupported slot-table size yields no rows.
    ///
    /// The item's display name and icon come from its `.itm` (and
    /// `dialog.tlk`), which the caller resolves — this only flattens the CRE's
    /// `item_slots` index table against its item list.
    pub fn inventory(&self, game: Game) -> Vec<InventoryRow> {
        let (slots, items) = slots_and_items(&self.cre);
        slot_labels(game, slots.len())
            .iter()
            .enumerate()
            .map(|(i, &position)| {
                let item = slots
                    .get(i)
                    .copied()
                    .filter(|&idx| idx >= 0)
                    .and_then(|idx| items.get(idx as usize))
                    .cloned();
                InventoryRow { position, item }
            })
            .collect()
    }
}

impl From<Cre> for ImportedCre {
    fn from(cre: Cre) -> Self {
        Self::new(cre)
    }
}

// `ImportedCre` is an owning smart pointer over the CRE: deref / `as_ref`
// give transparent access to the wrapped [`Cre`] (its parser-level fields
// and methods), while the inherent methods above add the resource-resolved
// queries. This lets it stand in wherever a `&Cre` was used before.
impl std::ops::Deref for ImportedCre {
    type Target = Cre;
    fn deref(&self) -> &Cre {
        &self.cre
    }
}

impl std::ops::DerefMut for ImportedCre {
    fn deref_mut(&mut self) -> &mut Cre {
        &mut self.cre
    }
}

impl AsRef<Cre> for ImportedCre {
    fn as_ref(&self) -> &Cre {
        &self.cre
    }
}

/// The name strref of the `KITLIST.2DA` row for a kit, given the high word of
/// the CRE kit dword: the mixed-case `MIXED` column, falling back to `LOWER`.
/// `None` when `high_word` isn't a `0x4000`-marked kit index, no row matches,
/// or the matched row carries no usable (non-zero) name strref.
///
/// `high_word` is `0x4000 | row_index` for a kitted creature; the row index
/// (after masking the marker) is the KITLIST row's numeric label. Row 0
/// ("RESERVE") is the no-kit placeholder and carries no name.
fn kit_strref_from_2da(kitlist: &TwoDA, high_word: u32) -> Option<u32> {
    // Must carry the "has kit" marker and nothing above it (a kitted dword is
    // `0x40XX0000`, so its high word is `0x40XX`); other forms aren't a kit
    // index.
    if high_word & 0xC000 != 0x4000 {
        return None;
    }
    let row_index = high_word & 0x3FFF;
    let row = kitlist.rows.get(&row_index.to_string())?;
    ["MIXED", "LOWER"]
        .into_iter()
        .find_map(|label| cells_strref(row, two_da_col(kitlist, label)?).filter(|&s| s != 0))
}

/// The `STRREF` of the `HATERACE.2DA` row whose `IDS` column equals `value`
/// (the racial-enemy byte). `None` when no row matches.
fn haterace_strref_from_2da(haterace: &TwoDA, value: u8) -> Option<u32> {
    let ids_col = two_da_col(haterace, "IDS")?;
    let strref_col = two_da_col(haterace, "STRREF")?;
    let row = haterace.rows.values().find(|cells| {
        cells.get(ids_col).and_then(|c| parse_2da_int(c)) == Some(i64::from(value))
    })?;
    cells_strref(row, strref_col).filter(|&s| s != 0)
}

/// Case-insensitive 2DA column index by header label.
fn two_da_col(two_da: &TwoDA, label: &str) -> Option<usize> {
    two_da
        .headers
        .iter()
        .position(|h| h.eq_ignore_ascii_case(label))
}

/// Parse a 2DA cell as an integer, accepting hex (`0x…`) or decimal.
fn parse_2da_int(cell: &str) -> Option<i64> {
    let cell = cell.trim();
    match cell.strip_prefix("0x").or_else(|| cell.strip_prefix("0X")) {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => cell.parse().ok(),
    }
}

/// A 2DA cell parsed as a non-negative strref (`u32`); `None` for `*`,
/// negative, or out-of-range values.
fn cells_strref(cells: &[String], col: usize) -> Option<u32> {
    parse_2da_int(cells.get(col)?).and_then(|n| u32::try_from(n).ok())
}

// ── Item-slot layout (Inventory tab) ────────────────────────────────
//
// A creature's `item_slots` table maps each equipment-slot position to an
// index into its item list (a negative index meaning "empty"). The slot
// *order* — what each position means — is game-specific, so the label lists
// below mirror the layouts NearInfinity / EEKeeper display.

/// The BG / BG2 / IWD (CRE V1.0 / V9.0) item-slot layout. The on-disk array
/// has 40 entries; the trailing two (the selected-weapon and selected-ability
/// indices) are not real item slots, so they're omitted — leaving 38 slots:
/// through the 16 inventory slots and ending at slot 37, the magically-created
/// weapon ("Magic Weapon").
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
    "Inventory",
    "Magic Weapon",
];

/// The Icewind Dale 2 (CRE V2.2) item-slot layout. The on-disk array has 52
/// entries; the trailing two (the selected-weapon and selected-weapon-ability
/// indices) are metadata, not real slots, so they're omitted — leaving 50
/// displayed slots ending at the magically-created weapon ("Magic Weapon").
///
/// IWD2 (3E) differs from the BG/IWD family: it has four weapon *sets*, each a
/// main-hand weapon followed by its off-hand/shield (interleaved, indices
/// 9–16), four quivers, then the cloak (index 21, after the quivers), three
/// quick items, and a 24-slot inventory grid. The ordering follows
/// NearInfinity's `readIWD2` (the on-disk order) and matches a real IWD2
/// save — note gemrb's `slottype.2da` SAVEORDER lists the cloak out of
/// sequence, so it isn't the on-disk index.
const IWD2_SLOT_LABELS: &[&str] = &[
    "Helmet",       // 0
    "Armor",        // 1
    "Shield",       // 2  (worn shield / paperdoll off-hand)
    "Gauntlets",    // 3
    "Ring",         // 4
    "Ring",         // 5
    "Amulet",       // 6
    "Belt",         // 7
    "Boots",        // 8
    "Weapon",       // 9   set 1 — main hand
    "Shield",       // 10  set 1 — off hand
    "Weapon",       // 11  set 2 — main hand
    "Shield",       // 12  set 2 — off hand
    "Weapon",       // 13  set 3 — main hand
    "Shield",       // 14  set 3 — off hand
    "Weapon",       // 15  set 4 — main hand
    "Shield",       // 16  set 4 — off hand
    "Quiver",       // 17
    "Quiver",       // 18
    "Quiver",       // 19
    "Quiver",       // 20  (GUI-inaccessible)
    "Cloak",        // 21
    "Quick Item",   // 22
    "Quick Item",   // 23
    "Quick Item",   // 24
    "Inventory",    // 25
    "Inventory",    // 26
    "Inventory",    // 27
    "Inventory",    // 28
    "Inventory",    // 29
    "Inventory",    // 30
    "Inventory",    // 31
    "Inventory",    // 32
    "Inventory",    // 33
    "Inventory",    // 34
    "Inventory",    // 35
    "Inventory",    // 36
    "Inventory",    // 37
    "Inventory",    // 38
    "Inventory",    // 39
    "Inventory",    // 40
    "Inventory",    // 41
    "Inventory",    // 42
    "Inventory",    // 43
    "Inventory",    // 44
    "Inventory",    // 45
    "Inventory",    // 46
    "Inventory",    // 47
    "Inventory",    // 48
    "Magic Weapon", // 49
];

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

/// Pick the slot-name list for this creature's game and slot-table size.
///
/// Planescape: Torment (both classic and EE) uses its own ordering (earrings,
/// tattoos, …) built to the creature's actual slot count — classic PST and
/// PST:EE differ, so each has its own list. Icewind Dale 2 (CRE V2.2) has its
/// own 52-entry layout (weapon sets, four quivers, …). Every other supported
/// engine uses the 40-slot BG / BG2 / IWD family layout; any other size falls
/// back to nothing so we show no rows rather than mislabel them.
fn slot_labels(game: Game, slot_count: usize) -> Vec<&'static str> {
    match game {
        Game::Pst => pst_slot_labels(slot_count),
        Game::Pstee => pstee_slot_labels(slot_count),
        Game::Iwd2 => IWD2_SLOT_LABELS.to_vec(),
        _ if slot_count == 40 => BG_SLOT_LABELS.to_vec(),
        _ => Vec::new(),
    }
}

/// Build the classic Planescape: Torment (CRE V1.2) slot labels for a
/// creature with `slot_count` on-disk item-slot entries.
///
/// The table has 48 entries whose trailing two are the selected-weapon and
/// selected-weapon-ability indices — metadata, not real slots — so they're
/// dropped, leaving 46 displayed slots ending at the magically-created weapon.
fn pst_slot_labels(slot_count: usize) -> Vec<&'static str> {
    let displayed = slot_count.saturating_sub(2);
    (0..displayed).map(pst_label).collect()
}

/// The classic-PST label for the item-slot at index `i`.
///
/// Follows NearInfinity's CRE V1.2 layout — note it differs from PST:EE (six
/// quivers, the upper tattoo placed after the quivers, five quick items,
/// twenty inventory slots).
fn pst_label(i: usize) -> &'static str {
    match i {
        0 => "Earring", // right earring / eye
        1 => "Chest",
        2 => "Tattoo", // left tattoo
        3 => "Hand",
        4 => "Ring",    // left ring
        5 => "Ring",    // right ring
        6 => "Earring", // left earring
        7 => "Tattoo",  // right tattoo (lower)
        8 => "Wrist",
        9..=12 => "Quick Weapon",
        13..=18 => "Quiver",     // six quiver slots in classic PST
        19 => "Tattoo",          // right tattoo (upper)
        20..=24 => "Quick Item", // five quick-item slots
        25..=44 => "Inventory",  // twenty inventory slots
        45 => "Magic Weapon",    // magically-created weapon
        _ => "Unused",
    }
}

/// Build the PST:EE slot labels for a creature with `slot_count` on-disk
/// item-slot entries.
///
/// PST:EE stores a variable-length slot table whose trailing two entries are
/// the selected-weapon and selected-weapon-ability indices — dropped here.
/// Unregistered creatures have the base 40-entry table (38 displayed slots,
/// ending at the magically-created weapon); registered party members gain
/// extra quiver / quick-item / inventory slots followed by unused padding.
fn pstee_slot_labels(slot_count: usize) -> Vec<&'static str> {
    let displayed = slot_count.saturating_sub(2);
    (0..displayed).map(pstee_label).collect()
}

/// The PST:EE label for the item-slot at index `i`.
fn pstee_label(i: usize) -> &'static str {
    match i {
        0 => "Earring",
        1 => "Chest",
        2 => "Tattoo",
        3 => "Hand",
        4 => "Ring",
        5 => "Ring",
        6 => "Earring",
        7 => "Tattoo",
        8 => "Wrist",
        9..=12 => "Quick Weapon",
        13..=16 => "Quiver",
        17 => "Tattoo",
        18..=20 => "Quick Item",
        21..=36 => "Inventory",
        37 => "Magic Weapon",
        // Registered party members gain these trailing slots:
        38 => "Quiver",
        39 => "Unused",
        40 => "Quick Item",
        41 => "Quick Item",
        42..=45 => "Inventory",
        _ => "Unused",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `TwoDA` from header labels and `(row_label, cells)` rows,
    /// mirroring the on-disk layout (cells align 1:1 with `headers`).
    fn make_two_da(headers: &[&str], rows: &[(&str, &[&str])]) -> TwoDA {
        TwoDA {
            headers: headers.iter().map(|s| s.to_string()).collect(),
            default: String::new(),
            rows: rows
                .iter()
                .map(|(label, cells)| {
                    (
                        label.to_string(),
                        cells.iter().map(|s| s.to_string()).collect(),
                    )
                })
                .collect(),
        }
    }

    #[test]
    fn parse_2da_int_handles_hex_and_decimal() {
        assert_eq!(parse_2da_int(" 0x00004006 "), Some(0x4006));
        assert_eq!(parse_2da_int("115"), Some(115));
        assert_eq!(parse_2da_int("*"), None);
    }

    #[test]
    fn two_da_col_is_case_insensitive() {
        let t = make_two_da(&["ROWNAME", "KITIDS"], &[]);
        assert_eq!(two_da_col(&t, "kitids"), Some(1));
        assert_eq!(two_da_col(&t, "missing"), None);
    }

    /// The kit dword's high word is `0x4000 | row_index`; the row is looked
    /// up by its numeric label (no `KITIDS` column needed — BG2 lacks one)
    /// and the mixed-case `MIXED` strref wins over `LOWER`. This mirrors the
    /// real BG2 KITLIST layout, where row 5 is the Inquisitor.
    #[test]
    fn kit_strref_matched_by_row_index_prefers_mixed_case() {
        let t = make_two_da(
            &[
                "ROWNAME",
                "LOWER",
                "MIXED",
                "HELP",
                "ABILITIES",
                "PROFICIENCY",
                "UNUSABLE",
                "CLASS",
            ],
            &[
                (
                    "6",
                    &[
                        "UNDEAD_HUNTER",
                        "9000", // LOWER
                        "9001", // MIXED
                        "0",
                        "CLABPA03",
                        "33",
                        "0x10",
                        "6",
                    ],
                ),
                (
                    "5",
                    &[
                        "INQUISITOR",
                        "9002",
                        "9003",
                        "0",
                        "CLABPA02",
                        "33",
                        "0x10",
                        "6",
                    ],
                ),
            ],
        );
        // 0x40060000 >> 16 == 0x4006 → row index 6 (Undead Hunter) → MIXED.
        assert_eq!(kit_strref_from_2da(&t, 0x4006), Some(9001));
        // Row 5 (Inquisitor) → its MIXED strref.
        assert_eq!(kit_strref_from_2da(&t, 0x4005), Some(9003));
        // No `0x4000` marker → not a kit index → None.
        assert_eq!(kit_strref_from_2da(&t, 0x9999), None);
        // Marked, but no such row.
        assert_eq!(kit_strref_from_2da(&t, 0x4020), None);
    }

    /// Falls back to the lowercase strref when the mixed-case one is 0.
    #[test]
    fn kit_strref_falls_back_to_lower() {
        let t = make_two_da(
            &["ROWNAME", "LOWER", "MIXED"],
            &[("1", &["FOO", "9000", "0"])],
        );
        // 0x4001 → row index 1; MIXED is 0 so LOWER (9000) is used.
        assert_eq!(kit_strref_from_2da(&t, 0x4001), Some(9000));
    }

    /// HATERACE matches by `IDS` (the RACE.IDS value) and returns the
    /// `STRREF`.
    #[test]
    fn haterace_strref_resolves_by_ids_value() {
        let t = make_two_da(
            &["STRREF", "IDS", "STRREF_HELP"],
            &[
                ("SKELETAL_UNDEAD", &["3283", "115", "3296"]),
                ("GOBLINS", &["3280", "161", "*"]),
            ],
        );
        assert_eq!(haterace_strref_from_2da(&t, 115), Some(3283));
        assert_eq!(haterace_strref_from_2da(&t, 200), None);
    }

    // ── Item-slot layout ─────────────────────────────────────────────

    /// The BG family exposes 38 item slots: 16 "Inventory" slots followed by
    /// the magically-created weapon ("Magic Weapon") at slot 37 — the same
    /// inventory/magic-weapon tail the PST:EE layout uses.
    #[test]
    fn bg_layout_has_16_inventory_then_magic_weapon() {
        assert_eq!(BG_SLOT_LABELS.len(), 38);
        assert_eq!(
            BG_SLOT_LABELS.iter().filter(|&&l| l == "Inventory").count(),
            16,
        );
        assert!(BG_SLOT_LABELS[21..=36].iter().all(|&l| l == "Inventory"));
        assert_eq!(BG_SLOT_LABELS[37], "Magic Weapon");
    }

    /// The BG labels agree with the PST:EE tail for the inventory + magic
    /// weapon region.
    #[test]
    fn bg_and_pstee_agree_on_inventory_tail() {
        for (i, item) in BG_SLOT_LABELS.iter().enumerate().take(37 + 1).skip(21) {
            assert_eq!(*item, pstee_label(i), "slot {i}");
        }
    }

    /// Classic PST (CRE V1.2) exposes 46 slots from its 48-entry table: six
    /// quivers, the upper tattoo after them, five quick items, twenty
    /// inventory slots, ending at the magically-created weapon.
    #[test]
    fn pst_classic_layout_matches_v1_2() {
        let labels = pst_slot_labels(48);
        assert_eq!(labels.len(), 46);
        assert_eq!(labels[0], "Earring");
        assert!(labels[13..=18].iter().all(|&l| l == "Quiver"));
        assert_eq!(labels[19], "Tattoo");
        assert_ne!(pstee_label(19), "Tattoo"); // PST:EE differs here
        assert!(labels[20..=24].iter().all(|&l| l == "Quick Item"));
        assert_eq!(labels.iter().filter(|&&l| l == "Inventory").count(), 20);
        assert!(labels[25..=44].iter().all(|&l| l == "Inventory"));
        assert_eq!(labels[45], "Magic Weapon");
    }

    /// IWD2 (CRE V2.2) exposes 50 slots from its 52-entry table: four
    /// interleaved weapon/shield sets (indices 9–16), four quivers, the cloak
    /// *after* the quivers (index 21, unlike the BG family), three quick
    /// items, a 24-slot inventory grid, ending at the magically-created
    /// weapon. A 52-entry table is selected purely by size.
    #[test]
    fn iwd2_layout_matches_v2_2() {
        assert_eq!(IWD2_SLOT_LABELS.len(), 50);
        assert_eq!(slot_labels(Game::Iwd2, 52), IWD2_SLOT_LABELS.to_vec());
        assert_eq!(IWD2_SLOT_LABELS[0], "Helmet");
        assert_eq!(IWD2_SLOT_LABELS[1], "Armor");
        // Weapon sets interleave a main-hand weapon with its off-hand shield.
        assert!((9..=15).step_by(2).all(|i| IWD2_SLOT_LABELS[i] == "Weapon"));
        assert!(
            (10..=16)
                .step_by(2)
                .all(|i| IWD2_SLOT_LABELS[i] == "Shield")
        );
        assert!(IWD2_SLOT_LABELS[17..=20].iter().all(|&l| l == "Quiver"));
        assert_eq!(IWD2_SLOT_LABELS[21], "Cloak"); // after the quivers
        assert!(IWD2_SLOT_LABELS[22..=24].iter().all(|&l| l == "Quick Item"));
        assert_eq!(
            IWD2_SLOT_LABELS
                .iter()
                .filter(|&&l| l == "Inventory")
                .count(),
            24
        );
        assert!(IWD2_SLOT_LABELS[25..=48].iter().all(|&l| l == "Inventory"));
        assert_eq!(IWD2_SLOT_LABELS[49], "Magic Weapon");
    }

    /// End-to-end: a real IWD2 (V2.2) creature resolves its carried items to
    /// the right slots — a wielded weapon in weapon-set-1 (index 9) and a
    /// stowed item in the inventory grid (index 31).
    #[test]
    fn iwd2_inventory_resolves_real_v2_2_cre() {
        use infinitier_cre_resource::CreImporter;
        use infinitier_datasource::{DataSource, Importer};

        let path = infinitier_test_utils::get_assets_path().join("cre/v2_2/52SERSA.cre");
        let cre = CreImporter {
            name: "52SERSA",
            game: Game::Iwd2,
        }
        .import(&DataSource::new(path.as_path()))
        .expect("import V2.2 CRE fixture");
        let rows = ImportedCre::new(cre).inventory(Game::Iwd2);

        assert_eq!(rows.len(), 50);
        let weapon = &rows[9];
        assert_eq!(weapon.position, "Weapon");
        assert_eq!(
            weapon.item.as_ref().expect("wielded weapon").item,
            "RAMO_MK"
        );
        let stowed = &rows[31];
        assert_eq!(stowed.position, "Inventory");
        assert_eq!(stowed.item.as_ref().expect("stowed item").item, "RT10_M");
        // Every other slot is empty for this creature.
        assert_eq!(rows.iter().filter(|r| r.item.is_some()).count(), 2);
    }
}
