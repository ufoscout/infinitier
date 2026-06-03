//! Read-only extraction of the save's GLOBAL script variables for the
//! Global Variables tab.
//!
//! Globals are stored once per save in the GAM (not per creature), so
//! every party member shows the same list — this reads
//! [`ImportedGam::variables`]. The displayed value is the variable's
//! integer slot (`int_value`, GAM offset 0x28), which is where the
//! engine keeps `GLOBAL` integers.

use infinitier_core::imported_resource::gam::ImportedGam;

/// One table row: a global variable's name and integer value.
pub struct GlobalVar<'a> {
    pub name: &'a str,
    pub value: i32,
}

/// Collect the GLOBAL variables, sorted the way EEKeeper presents them.
pub fn global_variable_rows(gam: &ImportedGam) -> Vec<GlobalVar<'_>> {
    let mut rows: Vec<GlobalVar<'_>> = gam
        .variables
        .iter()
        .map(|v| GlobalVar {
            name: &v.name,
            value: v.int_value,
        })
        .collect();
    rows.sort_by_cached_key(|r| collation_key(r.name));
    rows
}

/// EEKeeper sorts the list case-insensitively with `_` ordering
/// *before* letters — e.g. `ACH_TO_HELL_AND_BACK` precedes
/// `ACH_TOOK_A_CHANCE`. Raw byte order would do the opposite (`_` is
/// 0x5F, after the letters). Reproduce EEKeeper's order by uppercasing
/// and remapping `_` to 0x40 (after digits, just before `A`).
fn collation_key(name: &str) -> Vec<u8> {
    name.bytes()
        .map(|b| match b.to_ascii_uppercase() {
            b'_' => b'@',
            c => c,
        })
        .collect()
}
