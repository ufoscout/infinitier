//! Spells tab — IWD2 (CRE V2.2) read-only spell list.
//!
//! Unlike the AD&D games (which split arcane / divine / innate across
//! separate tabs), IWD2 keeps every spell in one tab with a dropdown
//! that selects the category. [`data`] flattens the chosen IWD2 spell
//! block into rows; [`view`] resolves each spell's display name and
//! paints the table.

mod data;
mod view;

use eframe::egui;
use egui_components::{Label, Select};
use infinitier_core::game::GameData;
use infinitier_core::resource::cre::Cre;

use self::data::SpellCategory;

pub struct SpellsTab;

impl SpellsTab {
    /// Needs `game_data` to resolve spell resrefs to display names via
    /// their SPL files and `dialog.tlk`. `char_key` identifies the
    /// creature being shown so the dropdown selection is remembered
    /// per-character (and re-defaulted when a different one is shown).
    pub fn show(&self, ui: &mut egui::Ui, cre: &Cre, char_key: u64, game_data: &GameData) {
        // How many spells the creature has of each category — used both
        // for the dropdown counts and to pick the default selection.
        let counts: Vec<usize> = SpellCategory::ALL
            .iter()
            .map(|(c, _)| data::spell_count(cre, *c))
            .collect();

        // The dropdown selection persists across frames in the egui frame
        // store, keyed by character. On the first view of a creature
        // (anchor mismatch) default to the first category it actually has
        // spells in, so the table isn't empty on open.
        let sel_id = egui::Id::new("iwd2_spell_category");
        let anchor_id = egui::Id::new("iwd2_spell_category_anchor");
        let anchor = ui.ctx().data_mut(|d| d.get_temp::<u64>(anchor_id));
        let mut selected = Some(if anchor == Some(char_key) {
            ui.ctx()
                .data_mut(|d| d.get_temp::<usize>(sel_id))
                .unwrap_or(0)
        } else {
            counts.iter().position(|&n| n > 0).unwrap_or(0)
        });

        ui.horizontal(|ui| {
            ui.add(Label::new("Spell Type").strong());
            ui.add_space(8.0);
            Select::new("iwd2_spell_category_select", &mut selected)
                .options(
                    SpellCategory::ALL
                        .iter()
                        .zip(&counts)
                        .map(|((_, label), &n)| {
                            if n > 0 {
                                format!("{label} ({n})")
                            } else {
                                label.to_string()
                            }
                        }),
                )
                .width(220.0)
                .show(ui);
        });

        let idx = selected.unwrap_or(0);
        ui.ctx().data_mut(|d| {
            d.insert_temp(sel_id, idx);
            d.insert_temp(anchor_id, char_key);
        });

        ui.add_space(8.0);
        let category = SpellCategory::ALL[idx].0;
        let rows = data::spell_rows(cre, category);
        view::render(ui, &rows, category, game_data);
    }
}
