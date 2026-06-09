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

use infinitier_cre_resource::Cre;
use infinitier_two_da_resource::TwoDA;

use crate::game::GameData;

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
}
