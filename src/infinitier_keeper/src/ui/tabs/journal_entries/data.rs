//! Read-only extraction for the Journal Entries tab.
//!
//! The party journal lives in the GAM file: one record per logged
//! entry, each pointing at a `dialog.tlk` strref for its text. The
//! `section` byte is a bitfield selecting which in-game journal page
//! the entry belongs to; EEKeeper shows it as the "Journal Type"
//! column. The `time` is the in-game timestamp (in ticks) the entry
//! was logged, rendered as a day / hour / minute clock.

use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GameTicks;

/// One row of the journal table.
pub struct JournalRow {
    /// Human-readable name for the `section` bitfield ("Journal Type").
    pub type_label: &'static str,
    /// Chapter the entry was logged in.
    pub chapter: u32,
    /// In-game time the entry was logged.
    pub time: GameTicks,
    /// `dialog.tlk` strref holding the entry's text.
    pub strref: u32,
}

/// Project the GAM journal into display rows, preserving file order.
pub fn journal_rows(gam: &ImportedGam) -> Vec<JournalRow> {
    gam.journal
        .iter()
        .map(|j| JournalRow {
            type_label: section_label(j.section),
            chapter: u32::from(j.chapter),
            time: j.time,
            strref: j.strref,
        })
        .collect()
}

/// Map the journal `section` bitfield to the page it belongs to.
///
/// The engine stores the destination page as a single set bit: the
/// active-quests page (1), the completed-quests page (2), the plain
/// journal/info page (4), or a player-written user note (no bit set).
/// The wording mirrors EEKeeper's terse "Journal Type" labels ("Info",
/// "Quest", …); anything unexpected falls back to "Other" so nothing is
/// silently hidden.
fn section_label(section: u8) -> &'static str {
    match section {
        0 => "User",
        1 => "Quest",
        2 => "Done Quest",
        4 => "Info",
        _ => "Other",
    }
}
