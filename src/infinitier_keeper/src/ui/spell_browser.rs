//! The floating **Spell Browser** window.
//!
//! The spell counterpart of [`ItemBrowser`](super::item_browser): a
//! scrollable, selectable table of every spell in the loaded game
//! (Type · Lvl · Name · Resource · Script Name), with the selected spell's
//! icon to the *left* of its description along the bottom. The right-hand
//! column carries the same live filters — a free-text box (matched
//! case-insensitively against type, name, resref and script name) and a
//! checkbox per spell type. Same style/settings as the Item Browser.
//!
//! All state and rendering for the window lives here; `app.rs` only owns
//! a [`SpellBrowser`] and a `bool` for whether the window is open.

use std::collections::BTreeSet;

use eframe::egui;
use egui_components::{Checkbox, Label, Table, TableColumn};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::resource::ResourceType;

/// Width reserved for the right-hand filter column.
const FILTER_W: f32 = 220.0;
/// Height of a single spell row.
const ROW_H: f32 = 22.0;
/// Side of the (square) description icon.
const ICON_SIDE: f32 = 64.0;
/// Font size for the window's title (smaller than the default heading, so
/// the title text and the title-bar it sizes are both compact).
const TITLE_SIZE: f32 = 14.0;
/// Stable id for the search box, so arrow-key navigation can tell when the
/// user is typing (and leave the arrows to the text field then).
const SEARCH_ID: &str = "spell_browser_search";

/// One row of the spell list, with everything needed to render the table
/// and — on selection — resolve the icon and description.
struct SpellEntry {
    /// The `.spl` resref (the "Resource" column).
    resref: String,
    /// Spell name (identified, falling back to the unidentified name; for
    /// spells the real name usually lives in the unidentified strref).
    name: String,
    /// Display type ("Wizard", "Cleric", "Innate", …) — see [`spell_type_name`].
    type_name: &'static str,
    /// Spell level as stored in the header (0 for level-less spells).
    level: u32,
    /// `SPELL.IDS` symbol, title-cased (the "Script Name" column); empty
    /// when the resref has no `SPELL.IDS` entry.
    script_name: String,
    /// Spellbook-icon BAM resref.
    icon: String,
    /// Resolved description strref (identified, falling back to
    /// unidentified) — looked up in `dialog.tlk` on selection.
    description_strref: u32,
}

/// State for the Spell Browser window. Persisted across frames by the app.
pub struct SpellBrowser {
    /// Every spell in the game, built once on first open (`game_data` is
    /// constant for the keeper's lifetime, so the index never goes stale).
    entries: Option<Vec<SpellEntry>>,
    /// Free-text filter (matched case-insensitively against type/name/resref/script).
    text: String,
    /// Spell types the user has *unticked*. Storing the excluded set (rather
    /// than the included one) means every type — including any only
    /// discovered later — defaults to shown.
    hidden_types: BTreeSet<&'static str>,
    /// Selected spell resref; drives the icon + description panel.
    selected: Option<String>,
    /// Cached texture for the selected spell's icon, keyed by icon resref so
    /// it's only re-decoded when the selection's icon actually changes.
    icon_cache: Option<(String, Option<egui::TextureHandle>)>,
}

impl SpellBrowser {
    pub fn new() -> Self {
        Self {
            entries: None,
            text: String::new(),
            hidden_types: BTreeSet::new(),
            selected: None,
            icon_cache: None,
        }
    }

    /// Paint the window when `open` is set. Movable / resizable / closable
    /// (the title-bar X clears `open`). The title is given an explicit small
    /// font: the title-bar height tracks the title's font height, so this
    /// shrinks both the text and the bar.
    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool, game_data: &GameData) {
        egui::Window::new(egui::RichText::new("Spell Browser").size(TITLE_SIZE))
            .open(open)
            .default_size([820.0, 600.0])
            .resizable(true)
            .show(ctx, |ui| self.ui(ui, game_data));
    }

    fn ui(&mut self, ui: &mut egui::Ui, game_data: &GameData) {
        if self.entries.is_none() {
            self.entries = Some(build_index(game_data));
        }
        // Own the index for this frame so the `&mut self` helpers below can
        // freely touch `self.selected` / `self.text` without aliasing it.
        let entries = self.entries.take().unwrap_or_default();

        let needle = self.text.trim().to_lowercase();
        let filtered: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| !self.hidden_types.contains(e.type_name))
            .filter(|(_, e)| matches_text(e, &needle))
            .map(|(i, _)| i)
            .collect();

        // Up/Down move the selection through the filtered list — unless the
        // user is typing in the search box, where the arrows belong to the
        // text field. Returns the new position so the table can scroll to it.
        let scroll_target = self.handle_arrow_keys(ui, &entries, &filtered);

        // Split the window into regions with nested panels. The description
        // gets ~half the window height, sized explicitly each frame rather
        // than via a resizable panel's `default_size` (eframe persists egui
        // memory, so a once-stored panel height would otherwise stick).
        let desc_h = (ui.available_height() * 0.5).clamp(240.0, 640.0);
        egui::Panel::bottom("spell_browser_desc")
            .resizable(false)
            .exact_size(desc_h)
            .show_inside(ui, |ui| self.description(ui, game_data, &entries));
        egui::Panel::right("spell_browser_filters")
            .resizable(false)
            .exact_size(FILTER_W)
            .show_inside(ui, |ui| self.filters(ui, &entries, filtered.len()));
        egui::CentralPanel::default()
            .show_inside(ui, |ui| self.spell_table(ui, &entries, &filtered, scroll_target));

        self.entries = Some(entries);
    }

    /// Move the selection up/down through `filtered` on arrow-key presses,
    /// returning the new position (an index *into `filtered`*) so the table
    /// can scroll it into view. No-op while the search box has focus.
    fn handle_arrow_keys(
        &mut self,
        ui: &egui::Ui,
        entries: &[SpellEntry],
        filtered: &[usize],
    ) -> Option<usize> {
        let typing = ui.ctx().memory(|m| m.has_focus(egui::Id::new(SEARCH_ID)));
        if typing || filtered.is_empty() {
            return None;
        }
        let (down, up) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::ArrowUp),
            )
        });
        if !down && !up {
            return None;
        }
        // Current position of the selection within the filtered list.
        let cur = self
            .selected
            .as_deref()
            .and_then(|sel| filtered.iter().position(|&idx| entries[idx].resref == sel));
        let next = match cur {
            None => 0,
            Some(p) if down => (p + 1).min(filtered.len() - 1),
            Some(p) => p.saturating_sub(1),
        };
        self.selected = Some(entries[filtered[next]].resref.clone());
        Some(next)
    }

    /// The selectable Type/Lvl/Name/Resource/Script-Name table (virtualised —
    /// only visible rows are built, so the full ~1600-spell list stays cheap).
    /// `scroll_target` (an index into `filtered`) keeps the keyboard-selected
    /// row in view when the user navigates with the arrow keys.
    fn spell_table(
        &mut self,
        ui: &mut egui::Ui,
        entries: &[SpellEntry],
        filtered: &[usize],
        scroll_target: Option<usize>,
    ) {
        let mut clicked: Option<usize> = None;
        let selected = self.selected.clone();
        let mut table = Table::new("spell_browser_list")
            .striped(true)
            .selectable(true)
            .row_height(ROW_H)
            .max_height(ui.available_height())
            .column(TableColumn::initial(72.0).clip(true).header("Type"))
            .column(TableColumn::initial(44.0).clip(true).header("Lvl"))
            .column(
                TableColumn::remainder()
                    .at_least(160.0)
                    .clip(true)
                    .header("Name"),
            )
            .column(TableColumn::initial(96.0).clip(true).header("Resource"))
            .column(
                TableColumn::remainder()
                    .at_least(160.0)
                    .clip(true)
                    .header("Script Name"),
            );
        if let Some(target) = scroll_target {
            table = table.scroll_to_row(target, Some(egui::Align::Center));
        }
        table.show(ui, |body| {
            body.rows(filtered.len(), |i, mut row| {
                let e = &entries[filtered[i]];
                row.selected(selected.as_deref() == Some(e.resref.as_str()));
                row.col(|ui| {
                    ui.add(Label::new(e.type_name));
                });
                row.col(|ui| {
                    ui.add(Label::new(e.level.to_string()));
                });
                row.col(|ui| {
                    ui.add(Label::new(e.name.as_str()));
                });
                row.col(|ui| {
                    ui.add(Label::new(e.resref.as_str()));
                });
                row.col(|ui| {
                    ui.add(Label::new(e.script_name.as_str()));
                });
                if row.response().clicked() {
                    clicked = Some(filtered[i]);
                }
            });
        });
        if let Some(idx) = clicked {
            self.selected = Some(entries[idx].resref.clone());
        }
    }

    /// The right-hand filter column: free-text search, a checkbox per spell
    /// type, and the current (filtered) spell count.
    fn filters(&mut self, ui: &mut egui::Ui, entries: &[SpellEntry], shown: usize) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label("Search");
                ui.add(
                    egui::TextEdit::singleline(&mut self.text)
                        .id(egui::Id::new(SEARCH_ID))
                        .hint_text("type, name, resource or script…")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                ui.separator();

                ui.label("Types");
                for t in distinct_types(entries) {
                    let mut on = !self.hidden_types.contains(t);
                    if ui.add(Checkbox::new(&mut on, t)).changed() {
                        if on {
                            self.hidden_types.remove(t);
                        } else {
                            self.hidden_types.insert(t);
                        }
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.label(format!("Spells: {shown}"));
            });
    }

    /// The bottom panel: the selected spell's icon to the left of its
    /// scrollable description.
    fn description(&mut self, ui: &mut egui::Ui, game_data: &GameData, entries: &[SpellEntry]) {
        let Some(entry) = self
            .selected
            .as_deref()
            .and_then(|sel| entries.iter().find(|e| e.resref == sel))
        else {
            ui.weak("Select a spell to see its description.");
            return;
        };

        let texture = self.icon_texture(ui.ctx(), game_data, entry);
        let description = game_data
            .dialog_tlk()
            .ok()
            .and_then(|t| t.get(entry.description_strref))
            .unwrap_or_default();

        ui.horizontal_top(|ui| {
            if let Some(tex) = texture {
                ui.add(
                    egui::Image::new(&tex)
                        .max_height(ICON_SIDE)
                        .max_width(ICON_SIDE)
                        .fit_to_original_size(1.0),
                );
                ui.add_space(8.0);
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if description.is_empty() {
                        ui.weak("(no description)");
                    } else {
                        ui.add(egui::Label::new(description).wrap());
                    }
                });
        });
    }

    /// The selected spell's icon texture, decoded/uploaded once and reused
    /// until the selection's icon resref changes.
    fn icon_texture(
        &mut self,
        ctx: &egui::Context,
        game_data: &GameData,
        entry: &SpellEntry,
    ) -> Option<egui::TextureHandle> {
        let want = entry.icon.as_str();
        let stale = self.icon_cache.as_ref().map(|(k, _)| k.as_str()) != Some(want);
        if stale {
            let tex = (!want.is_empty())
                .then(|| load_icon(ctx, game_data, want))
                .flatten();
            self.icon_cache = Some((want.to_owned(), tex));
        }
        self.icon_cache.as_ref().and_then(|(_, t)| t.clone())
    }
}

/// Whether an entry matches the (already-lowercased) free-text needle.
fn matches_text(e: &SpellEntry, needle: &str) -> bool {
    needle.is_empty()
        || e.type_name.to_lowercase().contains(needle)
        || e.name.to_lowercase().contains(needle)
        || e.resref.to_lowercase().contains(needle)
        || e.script_name.to_lowercase().contains(needle)
}

/// The distinct type names present, sorted, for the checkbox list.
fn distinct_types(entries: &[SpellEntry]) -> BTreeSet<&'static str> {
    entries.iter().map(|e| e.type_name).collect()
}

/// Build the full spell index: parse every `.spl`, resolve its name,
/// description strref and `SPELL.IDS` script name, and project the row
/// fields. Sorted by type, then level, then name. Built once per window
/// lifetime.
fn build_index(game_data: &GameData) -> Vec<SpellEntry> {
    let tlk = game_data.dialog_tlk().ok();
    let spell_ids = game_data.import_ids_by_name("SPELL").ok();

    // Collect resrefs first so the borrow of `game_data` for the resource
    // index ends before we start importing each spell.
    let mut resrefs: Vec<String> = game_data
        .get_all_resources_by_type(ResourceType::Spl)
        .map(|r| r.name.clone())
        .collect();
    resrefs.sort();
    resrefs.dedup();

    let mut out = Vec::with_capacity(resrefs.len());
    for resref in resrefs {
        let Ok(spl) = game_data.import_spl_by_name(&resref) else {
            continue;
        };

        // Spells keep their real name/description in the *unidentified*
        // strref (the identified one is usually `0xFFFFFFFF`, or — for a few
        // spells — a valid-but-empty entry), so prefer it and fall back to
        // the identified strref only when it's missing. This matches the
        // keeper's Wizard/Cleric/Innate tabs.
        let name_ref = first_strref(
            spl.header.name_strref(),
            spl.header.name_identified_strref(),
        );
        let name = tlk
            .as_deref()
            .and_then(|t| t.get(name_ref))
            .unwrap_or_default();

        out.push(SpellEntry {
            type_name: spell_type_name(spl.header.spell_type()),
            level: spl.header.spell_level(),
            name,
            script_name: script_name(spell_ids.as_deref(), &resref),
            icon: spl.header.spellbook_icon().to_owned(),
            description_strref: first_strref(
                spl.header.description_strref(),
                spl.header.description_identified_strref(),
            ),
            resref,
        });
    }

    out.sort_by(|a, b| {
        a.type_name
            .cmp(b.type_name)
            .then(a.level.cmp(&b.level))
            .then_with(|| a.name.cmp(&b.name))
    });
    out
}

/// The `primary` strref, falling back to `fallback` when it's empty / the
/// no-string sentinel (`0xFFFFFFFF`).
fn first_strref(primary: u32, fallback: u32) -> u32 {
    if primary == 0 || primary == 0xFFFF_FFFF {
        fallback
    } else {
        primary
    }
}

/// The "Script Name" for a spell: its `SPELL.IDS` symbol title-cased
/// (`SPPR101` → `1101` → `CLERIC_BLESS` → `"Cleric Bless"`), or empty when
/// the resref isn't a spell-code resref or has no `SPELL.IDS` entry.
fn script_name(spell_ids: Option<&infinitier_core::resource::ids::Ids>, resref: &str) -> String {
    spell_ids
        .zip(spell_ids_value(resref))
        .and_then(|(ids, value)| ids.of_value(value))
        .map(title_case)
        .unwrap_or_default()
}

/// Map a spell resref to its `SPELL.IDS` numeric value: the two-letter
/// school code after `SP` selects a thousands digit (PR→1 priest, WI→2
/// wizard, IN→3 innate, CL→4 special) and the trailing number is added
/// (e.g. `SPWI923` → 2923). `None` for non-spell resrefs.
fn spell_ids_value(resref: &str) -> Option<i32> {
    let body = resref
        .get(..2)
        .filter(|p| p.eq_ignore_ascii_case("SP"))
        .map(|_| &resref[2..])?;
    let (code, number) = body.split_at_checked(2)?;
    let thousands = match code.to_ascii_uppercase().as_str() {
        "PR" => 1,
        "WI" => 2,
        "IN" => 3,
        "CL" => 4,
        _ => return None,
    };
    let n: i32 = number.parse().ok()?;
    Some(thousands * 1000 + n)
}

/// Title-case a `SPELL.IDS` symbol: `CLERIC_ARMOR_OF_FAITH` →
/// `Cleric Armor Of Faith`.
fn title_case(symbol: &str) -> String {
    symbol
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Load a spellbook-icon BAM, composite its first frame and upload it as an
/// egui texture. Mirrors the Item Browser's loader.
fn load_icon(ctx: &egui::Context, game_data: &GameData, icon: &str) -> Option<egui::TextureHandle> {
    let imported = game_data
        .import_by_name_and_type(icon, ResourceType::Bam)
        .ok()?;
    let ImportedResource::Bam(bam) = imported.as_ref() else {
        return None;
    };
    let frame = bam.render_frame_centered(0, 0)?;
    let size = [frame.width() as usize, frame.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, &frame.into_raw());
    Some(ctx.load_texture(format!("spell-icon/{icon}"), color, egui::TextureOptions::LINEAR))
}

/// Map an SPL spell-type id to a display "Type". Mirrors EEKeeper, which
/// shows priest spells as "Cleric". Unknown values fall back to "Other".
fn spell_type_name(spell_type: u16) -> &'static str {
    match spell_type {
        1 => "Wizard",
        2 => "Cleric",
        3 => "Psionic",
        4 => "Innate",
        5 => "Bard",
        0 => "Special",
        _ => "Other",
    }
}
