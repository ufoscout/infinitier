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
    /// The CRE kit dword is word-swapped to its `KITIDS` value before
    /// matching, and the match is on `KITIDS` (not the row label) because the
    /// `KIT.IDS` symbol and the KITLIST `ROWNAME` don't always agree
    /// (`UNDEADHUNTER` vs `UNDEAD_HUNTER`). Resolving the strref to a string
    /// against `dialog.tlk` is the caller's job (the UI owns the tlk cache).
    pub fn kit_strref(&self, game_data: &GameData) -> Option<u32> {
        let kit = self.cre.kit()?;
        let swapped = ((kit & 0xFFFF) << 16) | (kit >> 16);
        let kitlist = game_data.import_2da_by_name("kitlist").ok()?;
        kit_strref_from_2da(&kitlist, swapped)
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

/// The name strref of the `KITLIST.2DA` row whose `KITIDS` column equals
/// `value` (the word-swapped kit dword): the mixed-case `MIXED` column,
/// falling back to `LOWER`. `None` when no row matches or it carries no
/// usable (non-zero) name strref.
fn kit_strref_from_2da(kitlist: &TwoDA, value: u32) -> Option<u32> {
    let kitids_col = two_da_col(kitlist, "KITIDS")?;
    let row = kitlist.rows.values().find(|cells| {
        cells.get(kitids_col).and_then(|c| parse_2da_int(c)) == Some(i64::from(value))
    })?;
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

    /// KITLIST matches by the `KITIDS` value (the `KIT.IDS` symbol and the
    /// `ROWNAME` disagree: `UNDEADHUNTER` vs `UNDEAD_HUNTER`) and returns the
    /// mixed-case name strref, preferring `MIXED` over `LOWER`.
    #[test]
    fn kit_strref_prefers_mixed_case_matched_by_kitids() {
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
                "KITIDS",
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
                        "0x00004006",
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
                        "0x00004005",
                    ],
                ),
            ],
        );
        assert_eq!(kit_strref_from_2da(&t, 0x4006), Some(9001));
        assert_eq!(kit_strref_from_2da(&t, 0x9999), None);
    }

    /// Falls back to the lowercase strref when the mixed-case one is 0.
    #[test]
    fn kit_strref_falls_back_to_lower() {
        let t = make_two_da(
            &["ROWNAME", "LOWER", "MIXED", "KITIDS"],
            &[("1", &["FOO", "9000", "0", "0x4010"])],
        );
        assert_eq!(kit_strref_from_2da(&t, 0x4010), Some(9000));
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
