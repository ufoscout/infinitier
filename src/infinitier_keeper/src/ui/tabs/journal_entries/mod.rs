//! The "Journal Entries" tab: the party's logged journal, read-only.
//!
//! The journal is a property of the savegame (GAM), not of any single
//! character, so this tab takes the imported GAM plus `GameData` for
//! resolving each entry's `dialog.tlk` text.

mod calendar;
mod data;
mod view;

use eframe::egui;

use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;

pub struct JournalEntriesTab;

impl JournalEntriesTab {
    pub fn show(&self, ui: &mut egui::Ui, gam: &ImportedGam, game_data: &GameData) {
        let rows = data::journal_rows(gam);
        view::render(ui, &rows, game_data);
    }
}
