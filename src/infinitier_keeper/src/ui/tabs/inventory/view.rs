//! Rendering for the Inventory tab.
//!
//! Mirrors EEKeeper's layout: a scrollable table with one row per
//! equipment slot — an inventory icon, the slot's "Position" name, the
//! item's charge/quantity triple, its identified name, and its `.itm`
//! resref. Empty slots show only their position. Rows are selectable; a
//! clicked row's slot index is returned so the host can record it as the
//! Item Browser's assignment target.
//!
//! Each item's name and icon come from its `.itm` file (and, for the
//! name, `dialog.tlk`). Loading an ITM/BAM and uploading a texture is
//! expensive, so both the per-item display info and the decoded icon
//! textures are memoised in the egui frame store and resolved at most
//! once.

use std::collections::{HashMap, HashSet};

use eframe::egui;

use egui_components::{Label, Table, TableColumn};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::ImportedResource;
use infinitier_core::imported_resource::cre::InventoryRow;
use infinitier_core::resource::ResourceType;
use infinitier_core::resource::itm::{Itm, ItmHeader};

/// The slot's `.itm` resref, or `""` when the slot is empty.
fn row_resref(row: &InventoryRow) -> &str {
    row.item.as_ref().map_or("", |it| it.item.as_str())
}

/// egui frame-store key for the resref → display-info cache.
const ITEM_CACHE: &str = "inventory_item_cache";
/// egui frame-store key for the icon-resref → texture cache.
const ICON_CACHE: &str = "inventory_icon_cache";
/// Height of a table row — tall enough to seat an inventory icon.
const ROW_H: f32 = 40.0;
/// Maximum table width, so it doesn't stretch across the whole tab.
const MAX_TABLE_W: f32 = 800.0;

/// Per-item resolved display info.
#[derive(Clone)]
struct ItemDisplay {
    /// Identified item name (falling back to the unidentified name).
    name: String,
    /// Inventory-icon BAM resref.
    icon: String,
}

/// What a frame of the inventory table produced.
#[derive(Default)]
pub struct InventoryEvent {
    /// Row (single-)clicked this frame → the slot to select.
    pub clicked_slot: Option<usize>,
    /// A *filled* row double-clicked this frame → its `.itm` resref, to
    /// reveal in the Item Browser.
    pub browse: Option<String>,
    /// A slot whose `quantity1/2/3` were edited this frame, with the new
    /// values to write back.
    pub quantity_edit: Option<(usize, [u16; 3])>,
    /// A slot whose "Delete" action was chosen this frame — its item is to be
    /// removed from the inventory.
    pub delete_slot: Option<usize>,
}

/// Render the inventory table, reporting the row clicked (to select its slot)
/// and any filled row double-clicked (to reveal that item in the browser).
pub fn render(
    ui: &mut egui::Ui,
    rows: &[InventoryRow],
    game_data: &GameData,
    selected: Option<usize>,
) -> InventoryEvent {
    let mut event = InventoryEvent::default();
    if rows.is_empty() {
        ui.add(Label::new(
            "Inventory is unavailable for this creature's format.",
        ));
        return event;
    }

    let items = resolve_items(ui, game_data, rows);
    let icons = resolve_icons(ui, game_data, &items);

    // Cap the table width without wrapping it in `allocate_ui_with_layout`:
    // that wrapper breaks the egui_extras row click-sense, so the selectable
    // rows never report clicks.
    ui.set_max_width(MAX_TABLE_W);
    Table::new("inventory")
        .selectable(true)
        .row_height(ROW_H)
        .header_height(ROW_H)
        .max_height(ui.available_height())
        .column(TableColumn::exact(50.0)) // icon
        .column(TableColumn::initial(120.0).header("Position"))
        .column(TableColumn::initial(140.0).header("Quantity"))
        .column(TableColumn::remainder().clip(true).header("Item"))
        .column(TableColumn::initial(110.0).clip(true).header("Resource"))
        .column(TableColumn::exact(64.0)) // per-row actions menu (extra width
        // keeps the left-aligned button clear of the table border/scrollbar)
        .show(ui, |body| {
            body.rows(rows.len(), |i, mut row| {
                let r = &rows[i];
                let resref = row_resref(r);
                let display = items.get(resref);

                row.selected(selected == Some(i));
                row.col(|ui| {
                    if let Some(tex) = display
                        .and_then(|d| icons.get(&d.icon))
                        .and_then(|t| t.as_ref())
                    {
                        ui.add(
                            egui::Image::new(tex)
                                .max_height(ROW_H - 4.0)
                                .max_width(ROW_H - 4.0)
                                .fit_to_original_size(1.0),
                        );
                    }
                });
                row.col(|ui| {
                    ui.add(Label::new(r.position));
                });
                row.col(|ui| {
                    // Editable quantity/charges triple for a filled slot.
                    // Added straight into the cell's left-to-right, vertically
                    // centered layout (no nested `horizontal`, which would
                    // top-align the inputs against the tall icon row).
                    if let Some(it) = &r.item {
                        let mut q = [it.quantity1, it.quantity2, it.quantity3];
                        // Size each box to the DragValue's *natural* height
                        // (text + button padding). Using a smaller height made
                        // the box overflow downwards past its centered slot, so
                        // it sat below the row's vertical centre.
                        let h = ui.text_style_height(&egui::TextStyle::Button)
                            + 2.0 * ui.spacing().button_padding.y;
                        ui.spacing_mut().item_spacing.x = 3.0;
                        let mut changed = false;
                        for v in &mut q {
                            changed |= ui.add_sized([34.0, h], egui::DragValue::new(v)).changed();
                        }
                        if changed {
                            event.quantity_edit = Some((i, q));
                        }
                    }
                });
                row.col(|ui| {
                    let name = display.map(|d| d.name.as_str()).unwrap_or("");
                    ui.add(Label::new(name));
                });
                row.col(|ui| {
                    ui.add(Label::new(resref));
                });
                row.col(|ui| {
                    // Per-row actions menu — only filled slots have an item to
                    // act on. A single "Delete" entry for now; deleting needs
                    // no confirmation.
                    if r.item.is_some() {
                        ui.menu_button("…", |ui| {
                            if ui.button("Delete").clicked() {
                                event.delete_slot = Some(i);
                                ui.close();
                            }
                        });
                    }
                });
                let resp = row.response();
                if resp.clicked() {
                    event.clicked_slot = Some(i);
                }
                // Double-clicking a filled slot reveals it in the Item Browser.
                if resp.double_clicked() && !resref.is_empty() {
                    event.browse = Some(resref.to_owned());
                }
            });
        });
    event
}

/// Resolve each filled slot's resref to its display info (identified
/// name + icon resref), memoising in the egui frame store. Each ITM is
/// parsed at most once.
fn resolve_items(
    ui: &mut egui::Ui,
    game_data: &GameData,
    rows: &[InventoryRow],
) -> HashMap<String, ItemDisplay> {
    let id = egui::Id::new(ITEM_CACHE);
    let mut cache: HashMap<String, ItemDisplay> =
        ui.ctx().data_mut(|d| d.get_temp(id)).unwrap_or_default();

    let misses: HashSet<&str> = rows
        .iter()
        .map(row_resref)
        .filter(|s| !s.is_empty() && !cache.contains_key(*s))
        .collect();
    if misses.is_empty() {
        return cache;
    }

    let tlk = game_data.dialog_tlk().ok();
    for resref in misses {
        let display = load_item(game_data, tlk.as_deref(), resref);
        cache.insert(resref.to_owned(), display);
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, cache.clone()));
    cache
}

/// Load an item's ITM and project out its identified name + icon resref.
fn load_item(
    game_data: &GameData,
    tlk: Option<&infinitier_core::resource::tlk::Tlk>,
    resref: &str,
) -> ItemDisplay {
    let Ok(itm) = game_data.import_itm_by_name(resref) else {
        return ItemDisplay {
            name: String::new(),
            icon: String::new(),
        };
    };

    let mut strref = itm.header.name_identified_strref();
    if strref == 0 || strref == 0xFFFF_FFFF {
        strref = itm.header.name_strref();
    }
    let name = tlk.and_then(|t| t.get(strref)).unwrap_or_default();

    ItemDisplay {
        name,
        icon: icon_resref(&itm).to_string(),
    }
}

/// The inventory-icon BAM resref lives at the same offset in every ITM
/// header version.
fn icon_resref(itm: &Itm) -> &str {
    match &itm.header {
        ItmHeader::V1(h) => &h.inventory_icon,
        ItmHeader::V1_1(h) => &h.inventory_icon,
        ItmHeader::V2(h) => &h.inventory_icon,
    }
}

/// Decode and upload each distinct inventory icon once, memoising the
/// resulting textures in the egui frame store. `None` is cached as a
/// sticky failure so a missing/unreadable BAM isn't retried every frame.
fn resolve_icons(
    ui: &mut egui::Ui,
    game_data: &GameData,
    items: &HashMap<String, ItemDisplay>,
) -> HashMap<String, Option<egui::TextureHandle>> {
    let id = egui::Id::new(ICON_CACHE);
    let mut cache: HashMap<String, Option<egui::TextureHandle>> =
        ui.ctx().data_mut(|d| d.get_temp(id)).unwrap_or_default();

    let misses: HashSet<&str> = items
        .values()
        .map(|d| d.icon.as_str())
        .filter(|s| !s.is_empty() && !cache.contains_key(*s))
        .collect();
    if misses.is_empty() {
        return cache;
    }

    // Build textures outside the data-store lock.
    for icon in misses {
        let tex = load_icon(ui.ctx(), game_data, icon);
        cache.insert(icon.to_owned(), tex);
    }
    ui.ctx().data_mut(|d| d.insert_temp(id, cache.clone()));
    cache
}

/// Load an inventory-icon BAM, composite its first frame, and upload it
/// as an egui texture.
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
        format!("inv-icon/{icon}"),
        color,
        egui::TextureOptions::LINEAR,
    ))
}
