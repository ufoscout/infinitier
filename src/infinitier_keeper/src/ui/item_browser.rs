//! The floating **Item Browser** window.
//!
//! Mirrors EEKeeper's item list: a scrollable, selectable table of every
//! item in the loaded game (Type · Name · Resource), with the selected
//! item's inventory icon to the *left* of its long description along the
//! bottom. The right-hand column replaces EEKeeper's action buttons with
//! live filters — a free-text box (matched case-insensitively against the
//! type, name and resref) and a checkbox per item type.
//!
//! All state and rendering for the window lives here; `app.rs` only owns
//! an [`ItemBrowser`] and a `bool` for whether the window is open.

use std::collections::BTreeSet;

use eframe::egui;
use egui_components::{Checkbox, Label, Table, TableColumn};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::resource::ResourceType;

/// Width reserved for the right-hand filter column.
const FILTER_W: f32 = 220.0;
/// Height of a single item row.
const ROW_H: f32 = 22.0;
/// Side of the (square) description icon.
const ICON_SIDE: f32 = 64.0;
/// Font size for the window's title (smaller than the default heading, so
/// the title text and the title-bar it sizes are both compact).
const TITLE_SIZE: f32 = 14.0;
/// Stable id for the search box, so arrow-key navigation can tell when the
/// user is typing (and leave the arrows to the text field then).
const SEARCH_ID: &str = "item_browser_search";

/// One row of the item list, with everything needed to render the table
/// and — on selection — resolve the icon and description.
struct ItemEntry {
    /// The `.itm` resref (the "Resource" column).
    resref: String,
    /// Identified item name (falling back to the unidentified name).
    name: String,
    /// Display category ("Sword", "Staff", …) — see [`item_type_name`].
    type_name: &'static str,
    /// Inventory-icon BAM resref.
    icon: String,
    /// Resolved description strref (identified, falling back to
    /// unidentified) — looked up in `dialog.tlk` on selection.
    description_strref: u32,
}

/// State for the Item Browser window. Persisted across frames by the app.
pub struct ItemBrowser {
    /// Every item in the game, built once on first open (`game_data` is
    /// constant for the keeper's lifetime, so the index never goes stale).
    entries: Option<Vec<ItemEntry>>,
    /// Free-text filter (matched case-insensitively against type/name/resref).
    text: String,
    /// Item types the user has *unticked*. Storing the excluded set (rather
    /// than the included one) means every type — including any only
    /// discovered later — defaults to shown.
    hidden_types: BTreeSet<&'static str>,
    /// Selected item resref; drives the icon + description panel.
    selected: Option<String>,
    /// Cached texture for the selected item's icon, keyed by icon resref so
    /// it's only re-decoded when the selection's icon actually changes.
    icon_cache: Option<(String, Option<egui::TextureHandle>)>,
}

impl ItemBrowser {
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
        egui::Window::new(egui::RichText::new("Item Browser").size(TITLE_SIZE))
            .open(open)
            .default_size([780.0, 600.0])
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

        // Split the window into regions with nested panels (manual
        // `allocate_ui` doesn't constrain the table's egui_extras layout).
        // Bottom = icon + description, right = filters, centre = the list.
        //
        // The description gets ~half the window height, sized explicitly each
        // frame rather than via a resizable panel's `default_size`: eframe
        // persists egui memory, so a once-stored panel height would otherwise
        // stick and ignore the default. Item descriptions are long, so this
        // keeps them readable and scales with the window.
        let desc_h = (ui.available_height() * 0.5).clamp(240.0, 640.0);
        egui::Panel::bottom("item_browser_desc")
            .resizable(false)
            .exact_size(desc_h)
            .show_inside(ui, |ui| self.description(ui, game_data, &entries));
        egui::Panel::right("item_browser_filters")
            .resizable(false)
            .exact_size(FILTER_W)
            .show_inside(ui, |ui| self.filters(ui, &entries, filtered.len()));
        egui::CentralPanel::default()
            .show_inside(ui, |ui| self.item_table(ui, &entries, &filtered, scroll_target));

        self.entries = Some(entries);
    }

    /// Move the selection up/down through `filtered` on arrow-key presses,
    /// returning the new position (an index *into `filtered`*) so the table
    /// can scroll it into view. No-op while the search box has focus.
    fn handle_arrow_keys(
        &mut self,
        ui: &egui::Ui,
        entries: &[ItemEntry],
        filtered: &[usize],
    ) -> Option<usize> {
        let typing = ui
            .ctx()
            .memory(|m| m.has_focus(egui::Id::new(SEARCH_ID)));
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

    /// The selectable Type/Name/Resource table (virtualised — only visible
    /// rows are built, so the full 2000-plus item list stays cheap).
    /// `scroll_target` (an index into `filtered`) keeps the keyboard-selected
    /// row in view when the user navigates with the arrow keys.
    fn item_table(
        &mut self,
        ui: &mut egui::Ui,
        entries: &[ItemEntry],
        filtered: &[usize],
        scroll_target: Option<usize>,
    ) {
        let mut clicked: Option<usize> = None;
        let selected = self.selected.clone();
        let mut table = Table::new("item_browser_list")
            .striped(true)
            .selectable(true)
            .row_height(ROW_H)
            .max_height(ui.available_height())
            .column(TableColumn::initial(96.0).clip(true).header("Type"))
            .column(
                TableColumn::remainder()
                    .at_least(180.0)
                    .clip(true)
                    .header("Name"),
            )
            .column(TableColumn::initial(110.0).clip(true).header("Resource"));
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
                    ui.add(Label::new(e.name.as_str()));
                });
                row.col(|ui| {
                    ui.add(Label::new(e.resref.as_str()));
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

    /// The right-hand filter column: free-text search, a checkbox per item
    /// type, and the current (filtered) item count.
    fn filters(&mut self, ui: &mut egui::Ui, entries: &[ItemEntry], shown: usize) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.label("Search");
                ui.add(
                    egui::TextEdit::singleline(&mut self.text)
                        .id(egui::Id::new(SEARCH_ID))
                        .hint_text("type, name or resource…")
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
                ui.label(format!("Items: {shown}"));
            });
    }

    /// The bottom panel: the selected item's icon to the left of its
    /// scrollable long description.
    fn description(&mut self, ui: &mut egui::Ui, game_data: &GameData, entries: &[ItemEntry]) {
        let Some(entry) = self
            .selected
            .as_deref()
            .and_then(|sel| entries.iter().find(|e| e.resref == sel))
        else {
            ui.weak("Select an item to see its description.");
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

    /// The selected item's icon texture, decoded/uploaded once and reused
    /// until the selection's icon resref changes.
    fn icon_texture(
        &mut self,
        ctx: &egui::Context,
        game_data: &GameData,
        entry: &ItemEntry,
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
fn matches_text(e: &ItemEntry, needle: &str) -> bool {
    needle.is_empty()
        || e.type_name.to_lowercase().contains(needle)
        || e.name.to_lowercase().contains(needle)
        || e.resref.to_lowercase().contains(needle)
}

/// The distinct type names present, sorted, for the checkbox list.
fn distinct_types(entries: &[ItemEntry]) -> BTreeSet<&'static str> {
    entries.iter().map(|e| e.type_name).collect()
}

/// Build the full item index: parse every `.itm`, resolve its identified
/// name and description strref, and project the row fields. Sorted by
/// type then name (EEKeeper's grouping). Built once per window lifetime.
fn build_index(game_data: &GameData) -> Vec<ItemEntry> {
    let tlk = game_data.dialog_tlk().ok();

    // Collect resrefs first so the borrow of `game_data` for the resource
    // index ends before we start importing each item.
    let mut resrefs: Vec<String> = game_data
        .get_all_resources_by_type(ResourceType::Itm)
        .map(|r| r.name.clone())
        .collect();
    resrefs.sort();
    resrefs.dedup();

    let mut out = Vec::with_capacity(resrefs.len());
    for resref in resrefs {
        let Ok(itm) = game_data.import_itm_by_name(&resref) else {
            continue;
        };

        let name_ref = first_strref(
            itm.header.name_identified_strref(),
            itm.header.name_strref(),
        );
        let name = tlk
            .as_deref()
            .and_then(|t| t.get(name_ref))
            .unwrap_or_default();

        out.push(ItemEntry {
            resref,
            name,
            type_name: item_type_name(itm.header.item_type()),
            icon: itm.header.inventory_icon().to_owned(),
            description_strref: first_strref(
                itm.header.description_identified_strref(),
                itm.header.description_strref(),
            ),
        });
    }

    out.sort_by(|a, b| a.type_name.cmp(b.type_name).then_with(|| a.name.cmp(&b.name)));
    out
}

/// Prefer the identified strref, falling back to the unidentified one when
/// it's empty / the no-string sentinel.
fn first_strref(identified: u32, unidentified: u32) -> u32 {
    if identified == 0 || identified == 0xFFFF_FFFF {
        unidentified
    } else {
        identified
    }
}

/// Load an inventory-icon BAM, composite its first frame and upload it as
/// an egui texture. Mirrors the Inventory tab's loader.
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
    Some(ctx.load_texture(format!("item-icon/{icon}"), color, egui::TextureOptions::LINEAR))
}

/// Map an ITM category id to a display "Type", following the standard
/// Infinity Engine item-category table (IESDP), with the several blade
/// categories collapsed to "Sword" and the quarterstaff to "Staff" to
/// match EEKeeper's grouping. Categories outside this table (some PST /
/// modded values) fall back to "Other".
fn item_type_name(item_type: u16) -> &'static str {
    match item_type {
        1 => "Amulet",
        2 => "Armor",
        3 => "Belt",
        4 => "Boots",
        5 => "Arrow",
        6 => "Bracers",
        7 => "Helmet",
        8 => "Key",
        9 => "Potion",
        10 => "Ring",
        11 => "Scroll",
        12 => "Shield",
        13 => "Food",
        14 => "Bullet",
        15 => "Bow",
        16 => "Dagger",
        17 => "Mace",
        18 => "Sling",
        19 | 20 | 60 | 72 => "Sword", // short / long / great / bastard
        21 => "Hammer",
        22 => "Morning Star",
        23 => "Flail",
        24 => "Dart",
        25 => "Axe",
        26 => "Staff", // quarterstaff
        27 => "Crossbow",
        28 => "Fist",
        29 => "Spear",
        30 => "Halberd",
        31 => "Bolt",
        32 => "Cloak",
        33 => "Gold",
        34 => "Gem",
        35 => "Wand",
        36 | 61 => "Container",
        37 => "Book",
        38 => "Familiar",
        39 => "Tattoo",
        40 => "Lens",
        41 | 47 => "Buckler",
        42 => "Candle",
        44 => "Club",
        48 => "Large Shield",
        49 => "Medium Shield",
        50 => "Notes",
        57 => "Small Shield",
        58 => "Telescope",
        59 => "Drink",
        63 => "Leather Armor",
        73 => "Scarf",
        0 => "Miscellaneous",
        _ => "Other",
    }
}
