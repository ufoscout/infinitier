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
use egui_components::{Button, Checkbox, Label, Table, TableColumn};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::imported_resource::spl::ImportedSpl;
use infinitier_core::resource::Engine;
use infinitier_core::resource::ResourceType;
use infinitier_core::resource::cre::Iwd2Spellbook;

/// What the user asked to add this frame. AD&D games carry just the resref
/// (the host derives the spellbook/level from the SPL); IWD2 carries the
/// exact placement the user picked from the per-book menu.
pub enum SpellAssign {
    Adnd(String),
    Iwd2 {
        book: Iwd2Spellbook,
        level: u16,
        index: u32,
    },
}

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
    /// Set when the selection was changed programmatically (revealed from a
    /// Spells-tab row): scroll the list to it on the next frame.
    scroll_to_selected: bool,
    /// An open IWD2 "pick a spellbook" menu, raised by double-clicking a row:
    /// the spell's resref and the screen position to anchor the menu at.
    book_menu: Option<(String, egui::Pos2)>,
}

impl SpellBrowser {
    pub fn new() -> Self {
        Self {
            entries: None,
            text: String::new(),
            hidden_types: BTreeSet::new(),
            selected: None,
            icon_cache: None,
            scroll_to_selected: false,
            book_menu: None,
        }
    }

    /// Reveal `resref` in the browser: select it and scroll the list to it on
    /// the next frame. Clears the text filter and re-shows the spell's type so
    /// it can't stay hidden behind a stale filter. Called by the host when a
    /// Spells-tab row is double-clicked. Mirrors [`ItemBrowser::select`].
    ///
    /// The resref is lower-cased to match the index: the browser keys spells
    /// by the resource name (always lower-case), whereas a CRE/2DA record may
    /// store the resref in whatever case it was written.
    pub fn select(&mut self, resref: String) {
        let resref = resref.to_lowercase();
        self.text.clear();
        if let Some(entries) = &self.entries
            && let Some(entry) = entries.iter().find(|e| e.resref == resref)
        {
            self.hidden_types.remove(entry.type_name);
        }
        self.selected = Some(resref);
        self.scroll_to_selected = true;
    }

    /// Paint the window when `open` is set. Movable / resizable / closable
    /// (the title-bar X clears `open`). The title is given an explicit small
    /// font: the title-bar height tracks the title's font height, so this
    /// shrinks both the text and the bar.
    ///
    /// `can_assign` is true when the host can add the selected spell to the
    /// current character (a creature is selected). `add_label` is the button's
    /// caption (e.g. "Add to Xan"). For AD&D a single button adds the spell;
    /// for IWD2 the button is a menu that lets the user pick the spellbook
    /// (Cleric / Paladin / Domain / …). Returns the assignment requested this
    /// frame.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        open: &mut bool,
        game_data: &GameData,
        can_assign: bool,
        add_label: &str,
    ) -> Option<SpellAssign> {
        egui::Window::new(egui::RichText::new("Spell Browser").size(TITLE_SIZE))
            .open(open)
            .default_size([820.0, 600.0])
            .resizable(true)
            .show(ctx, |ui| self.ui(ui, game_data, can_assign, add_label))
            .and_then(|r| r.inner)
            .flatten()
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        game_data: &GameData,
        can_assign: bool,
        add_label: &str,
    ) -> Option<SpellAssign> {
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
        let mut scroll_target = self.handle_arrow_keys(ui, &entries, &filtered);
        // A programmatic select (revealed from a Spells-tab row) scrolls the
        // list to the now-selected spell this frame.
        if self.scroll_to_selected {
            self.scroll_to_selected = false;
            if let Some(sel) = self.selected.as_deref() {
                scroll_target = filtered.iter().position(|&i| entries[i].resref == sel);
            }
        }

        // Split the window into regions with nested panels. The description
        // gets ~half the window height, sized explicitly each frame rather
        // than via a resizable panel's `default_size` (eframe persists egui
        // memory, so a once-stored panel height would otherwise stick).
        // Collected this frame: the assignment the user asked for (via the
        // button/menu or a double-click), if any.
        let mut assign: Option<SpellAssign> = None;

        let desc_h = (ui.available_height() * 0.5).clamp(240.0, 640.0);
        egui::Panel::bottom("spell_browser_desc")
            .resizable(false)
            .exact_size(desc_h)
            .show_inside(ui, |ui| {
                if let Some(r) = self.description(ui, game_data, &entries, can_assign, add_label) {
                    assign = Some(r);
                }
            });
        egui::Panel::right("spell_browser_filters")
            .resizable(false)
            .exact_size(FILTER_W)
            .show_inside(ui, |ui| self.filters(ui, &entries, filtered.len()));
        let iwd2 = game_data.game().engine() == Engine::Iwd2;
        // A row double-clicked this frame opens its book menu (IWD2) at the
        // pointer; the trigger frame is flagged so the popup opens then.
        let mut opened_book_menu = false;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some((resref, pos)) =
                self.spell_table(ui, &entries, &filtered, scroll_target, can_assign)
            {
                if iwd2 {
                    // Ambiguous spellbook → show the same per-book menu the
                    // "Add" button uses, anchored at the double-click.
                    self.book_menu = Some((resref, pos));
                    opened_book_menu = true;
                } else {
                    // AD&D: one unambiguous spellbook, add straight away.
                    assign = Some(SpellAssign::Adnd(resref));
                }
            }
        });
        if let Some(picked) = self.book_menu_popup(ui, game_data, opened_book_menu) {
            assign = Some(picked);
        }

        self.entries = Some(entries);
        assign
    }

    /// Render the at-pointer "pick a spellbook" popup for a double-clicked IWD2
    /// row (when [`Self::book_menu`] is set). `just_opened` requests the popup
    /// open this frame. Returns the chosen placement; the popup closes on a
    /// pick or a click outside.
    fn book_menu_popup(
        &mut self,
        ui: &egui::Ui,
        game_data: &GameData,
        just_opened: bool,
    ) -> Option<SpellAssign> {
        let (resref, pos) = self.book_menu.clone()?;
        let placements = ImportedSpl::iwd2_placements(game_data, &resref);
        let popup_id = egui::Id::new("spell_book_menu");
        let open = just_opened.then_some(egui::SetOpenCommand::Bool(true));

        let mut picked = None;
        egui::Popup::new(
            popup_id,
            ui.ctx().clone(),
            egui::PopupAnchor::Position(pos),
            ui.layer_id(),
        )
        .open_memory(open)
        .kind(egui::PopupKind::Menu)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .show(|ui| {
            for p in &placements {
                if ui
                    .button(format!("{} — Lvl {}", book_name(p.book), p.level))
                    .clicked()
                {
                    picked = Some(SpellAssign::Iwd2 {
                        book: p.book,
                        level: p.level,
                        index: p.index,
                    });
                }
            }
        });

        // Forget the menu once a placement is chosen or it's closed (outside
        // click), so it doesn't re-render stale next frame. Don't clear on the
        // opening frame (the open command only takes effect during `show`).
        if picked.is_some() || (!just_opened && !egui::Popup::is_id_open(ui.ctx(), popup_id)) {
            self.book_menu = None;
        }
        picked
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
        can_assign: bool,
    ) -> Option<(String, egui::Pos2)> {
        let mut clicked: Option<usize> = None;
        // A double-clicked row (when assignment is possible) requests its
        // spell be added — with the pointer position, so IWD2 can open its
        // book menu right there.
        let mut assign: Option<(usize, egui::Pos2)> = None;
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
                let resp = row.response();
                if resp.clicked() {
                    clicked = Some(filtered[i]);
                }
                if can_assign && resp.double_clicked() {
                    let pos = resp
                        .interact_pointer_pos()
                        .unwrap_or_else(|| resp.rect.center());
                    assign = Some((filtered[i], pos));
                }
            });
        });
        if let Some(idx) = clicked {
            self.selected = Some(entries[idx].resref.clone());
        }
        assign.map(|(idx, pos)| (entries[idx].resref.clone(), pos))
    }

    /// The right-hand filter column: free-text search, a checkbox per spell
    /// type, and the current (filtered) spell count.
    fn filters(&mut self, ui: &mut egui::Ui, entries: &[SpellEntry], shown: usize) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label(format!("Spells: {shown}"));
                ui.add_space(8.0);
                ui.separator();

                ui.label("Search");
                ui.add(
                    egui::TextEdit::singleline(&mut self.text)
                        .id(egui::Id::new(SEARCH_ID))
                        .hint_text("type, name, resource or script…")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);
                ui.separator();

                // Master "select all types" checkbox heading the list.
                let mut all = self.hidden_types.is_empty();
                if ui.add(Checkbox::new(&mut all, "Types")).changed() {
                    if all {
                        self.hidden_types.clear();
                    } else {
                        self.hidden_types = distinct_types(entries);
                    }
                }
                ui.indent("spell_types", |ui| {
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
                });
            });
    }

    /// The bottom panel: the add-to-character control, then the selected
    /// spell's icon to the left of its scrollable description. For AD&D the
    /// control is a button; for IWD2 it's a menu listing every spellbook the
    /// spell can go into (Cleric / Paladin / Domain / …). Returns the
    /// assignment when the user requests one.
    fn description(
        &mut self,
        ui: &mut egui::Ui,
        game_data: &GameData,
        entries: &[SpellEntry],
        can_assign: bool,
        add_label: &str,
    ) -> Option<SpellAssign> {
        let entry = self
            .selected
            .as_deref()
            .and_then(|sel| entries.iter().find(|e| e.resref == sel));

        let mut request = None;
        if game_data.game().engine() == Engine::Iwd2 {
            // IWD2: one entry per spellbook the spell belongs to — let the
            // user pick (a wizard spell, a cleric/paladin/domain spell, …).
            let placements = entry
                .map(|e| ImportedSpl::iwd2_placements(game_data, &e.resref))
                .unwrap_or_default();
            ui.add_enabled_ui(can_assign && !placements.is_empty(), |ui| {
                ui.menu_button(add_label, |ui| {
                    for p in &placements {
                        let label = format!("{} — Lvl {}", book_name(p.book), p.level);
                        if ui.button(label).clicked() {
                            request = Some(SpellAssign::Iwd2 {
                                book: p.book,
                                level: p.level,
                                index: p.index,
                            });
                            ui.close();
                        }
                    }
                });
            });
        } else {
            // AD&D: one unambiguous spellbook (derived from the SPL).
            let enabled = can_assign && entry.is_some();
            if ui
                .add_enabled(enabled, Button::primary(add_label).small())
                .clicked()
                && let Some(e) = entry
            {
                request = Some(SpellAssign::Adnd(e.resref.clone()));
            }
        }
        ui.add_space(6.0);

        let Some(entry) = entry else {
            ui.weak("Select a spell to see its description.");
            return request;
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
        request
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
    Some(ctx.load_texture(
        format!("spell-icon/{icon}"),
        color,
        egui::TextureOptions::LINEAR,
    ))
}

/// Display name of an IWD2 spellbook, for the per-book add menu.
fn book_name(book: Iwd2Spellbook) -> &'static str {
    match book {
        Iwd2Spellbook::Bard => "Bard",
        Iwd2Spellbook::Cleric => "Cleric",
        Iwd2Spellbook::Druid => "Druid",
        Iwd2Spellbook::Paladin => "Paladin",
        Iwd2Spellbook::Ranger => "Ranger",
        Iwd2Spellbook::Sorcerer => "Sorcerer",
        Iwd2Spellbook::Wizard => "Wizard",
        Iwd2Spellbook::Domain => "Domain",
        Iwd2Spellbook::Innate => "Innate",
        Iwd2Spellbook::Song => "Song",
        Iwd2Spellbook::ShapeChange => "Shape",
    }
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
