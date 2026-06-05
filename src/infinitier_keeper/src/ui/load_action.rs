//! "Load" action for the egui keeper — the *Open Single Player Saved
//! Game* picker.
//!
//! Mirrors EEKeeper's load dialog: the header's Load button pops a modal
//! listing every discovered save game (folder name + the time it was
//! last written), sorted newest-first. Selecting a row previews its
//! screenshot and party portraits. Three actions: **Load** opens the
//! selected save into a tab, **Cancel** closes the dialog, and
//! **Delete** removes the save folder from disk after a confirmation
//! prompt.
//!
//! The list and previews are read fresh from disk each time the dialog
//! opens (and after a delete), so it always reflects the real save
//! directory. Loading mirrors the startup path in `main.rs`: import the
//! GAM through its `DataSource`, resolve it via [`ImportedGam::load`],
//! and push a new [`SaveTab`].

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use egui_components::{AlertChoice, AlertDialog, Button, Dialog, Label, LabelTone};
use egui_extras::{Column, TableBuilder};

use infinitier_core::fs::{DataSource, Importer};
use infinitier_core::game::GameData;
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::imported_resource::image::ImportedImage;
use infinitier_core::resource::bmp::BmpImporter;
use infinitier_core::resource::gam::GamImporter;
use infinitier_core::save_games::SaveGame;
use log::info;

use crate::state::{AppState, SaveTab};

/// One discovered save game, with its display timestamp precomputed.
struct SaveEntry {
    save: SaveGame,
    /// Newest file mtime in the save folder, used for sorting.
    modified: SystemTime,
    /// `modified` formatted like EEKeeper's "Time Saved" column.
    time_label: String,
}

/// Owned state for the load picker. Lives on `KeeperApp` so it survives
/// across frames (egui's immediate mode means the host holds the
/// dialog's open flag, scanned list, and selection).
#[derive(Default)]
pub struct LoadAction {
    open: bool,
    /// Discovered saves, newest first. Rebuilt on open and after delete.
    entries: Vec<SaveEntry>,
    /// Index into [`Self::entries`] of the highlighted row.
    selected: Option<usize>,
    /// Which entry the cached preview textures belong to (so we only
    /// reload them when the selection actually changes).
    preview_for: Option<usize>,
    screenshot: Option<egui::TextureHandle>,
    portraits: Vec<egui::TextureHandle>,
    /// Whether the delete-confirmation alert is showing.
    confirm_delete: bool,
    /// Last error (failed load / delete), surfaced inline.
    error: Option<String>,
}

impl LoadAction {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the picker, scanning the save directory fresh.
    pub fn open(&mut self, state: &AppState) {
        self.error = None;
        self.confirm_delete = false;
        self.rescan(&state.game_data);
        self.selected = (!self.entries.is_empty()).then_some(0);
        self.preview_for = None;
        self.screenshot = None;
        self.portraits.clear();
        self.open = true;
    }

    /// Paint the dialog if open and resolve whatever the user clicked.
    pub fn show(&mut self, ctx: &egui::Context, state: &mut AppState) {
        if !self.open {
            return;
        }
        self.sync_preview(ctx);

        let mut action = None;
        {
            // Split-borrow so the body can read the list / textures and
            // mutate the selection while `Dialog::show` holds `&mut open`.
            let LoadAction {
                open,
                entries,
                selected,
                screenshot,
                portraits,
                error,
                ..
            } = self;
            Dialog::new("Open Single Player Saved Game")
                .width(680.0)
                .show(ctx, open, |ui| {
                    action = render_body(
                        ui,
                        entries,
                        selected,
                        screenshot.as_ref(),
                        portraits,
                        error.as_deref(),
                    );
                });
        }

        match action {
            Some(DialogAction::Cancel) => self.open = false,
            Some(DialogAction::Load) => self.do_load(state),
            Some(DialogAction::Delete) if self.selected.is_some() => {
                self.error = None;
                self.confirm_delete = true;
            }
            Some(DialogAction::Delete) => {}
            None => {}
        }

        if self.confirm_delete {
            self.show_delete_confirm(ctx, state);
        }
    }

    /// Reload the screenshot + portrait thumbnails when the selection
    /// changed since they were last built.
    fn sync_preview(&mut self, ctx: &egui::Context) {
        if self.preview_for == self.selected {
            return;
        }
        self.screenshot = None;
        self.portraits.clear();
        if let Some(entry) = self.selected.and_then(|i| self.entries.get(i)) {
            let tag = entry.save.name.as_str();
            self.screenshot = entry
                .save
                .screenshot
                .as_ref()
                .and_then(|ds| load_bmp_texture(ctx, ds, &format!("save-shot/{tag}")));
            self.portraits = entry
                .save
                .portraits
                .iter()
                .enumerate()
                .filter_map(|(k, ds)| {
                    load_bmp_texture(ctx, ds, &format!("save-portrait/{tag}/{k}"))
                })
                .collect();
        }
        self.preview_for = self.selected;
    }

    /// Open the selected save into a tab (switching to it if already
    /// open), mirroring `main.rs`'s startup load.
    fn do_load(&mut self, state: &mut AppState) {
        let Some(save) = self
            .selected
            .and_then(|i| self.entries.get(i))
            .map(|e| &e.save)
        else {
            return;
        };

        // Already open? Just focus that tab.
        if let Some(idx) = state
            .tabs
            .iter()
            .position(|t| t.save_folder_path == save.folder_path)
        {
            state.active_tab = idx;
            self.open = false;
            return;
        }

        let engine = state.game_data.game().engine();
        let gam = match (GamImporter {
            name: &save.name,
            engine,
        })
        .import(&save.gam)
        {
            Ok(gam) => gam,
            Err(e) => {
                self.error = Some(format!(
                    "Couldn't read \"{}\": {e}",
                    display_name(&save.name)
                ));
                return;
            }
        };
        let imported = match ImportedGam::load(gam, &state.game_data) {
            Ok(imported) => imported,
            Err(e) => {
                self.error = Some(format!(
                    "Couldn't load \"{}\": {e}",
                    display_name(&save.name)
                ));
                return;
            }
        };

        let tab = SaveTab::new(
            save.name.clone(),
            save.folder_path.clone(),
            Box::new(imported),
        );
        info!("[load] opened save {}", save.folder_path.display());
        state.tabs.push(tab);
        state.active_tab = state.tabs.len() - 1;
        self.open = false;
    }

    /// Confirmation alert for Delete. On confirm, remove the folder and
    /// rescan; the in-memory tab (if any) is left untouched.
    fn show_delete_confirm(&mut self, ctx: &egui::Context, state: &mut AppState) {
        let Some(name) = self
            .selected
            .and_then(|i| self.entries.get(i))
            .map(|e| display_name(&e.save.name).to_string())
        else {
            self.confirm_delete = false;
            return;
        };
        let mut open = true;
        let choice = AlertDialog::new("Delete saved game")
            .description(format!(
                "Permanently delete \"{name}\"? This removes the save folder from disk and cannot be undone."
            ))
            .confirm_label("Delete")
            .cancel_label("Cancel")
            .danger()
            .show(ctx, &mut open);
        match choice {
            Some(AlertChoice::Confirm) => {
                self.confirm_delete = false;
                self.do_delete(state);
            }
            Some(AlertChoice::Cancel) => self.confirm_delete = false,
            None => {}
        }
    }

    /// Delete the selected save folder, then rescan and re-anchor the
    /// selection.
    fn do_delete(&mut self, state: &mut AppState) {
        let Some(folder) = self
            .selected
            .and_then(|i| self.entries.get(i))
            .map(|e| e.save.folder_path.clone())
        else {
            return;
        };
        if let Err(e) = std::fs::remove_dir_all(&folder) {
            self.error = Some(format!("Couldn't delete save: {e}"));
            return;
        }
        info!("[load] deleted save folder {}", folder.display());
        let prev = self.selected.unwrap_or(0);
        self.rescan(&state.game_data);
        self.selected = if self.entries.is_empty() {
            None
        } else {
            Some(prev.min(self.entries.len() - 1))
        };
        self.preview_for = None;
    }

    /// Rescan the save directory and rebuild [`Self::entries`], sorted
    /// by write time descending.
    fn rescan(&mut self, game_data: &GameData) {
        let mut entries: Vec<SaveEntry> = game_data
            .save_games()
            .saves()
            .iter()
            .map(|save| {
                let modified = folder_mtime(&save.folder_path);
                SaveEntry {
                    save: save.clone(),
                    modified,
                    time_label: format_time(modified),
                }
            })
            .collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.modified));
        self.entries = entries;
    }
}

/// Action chosen in the dialog body, resolved after `Dialog::show`
/// releases its borrows.
enum DialogAction {
    Cancel,
    Load,
    Delete,
}

/// Paint the list + preview + buttons; return the action clicked, if any.
fn render_body(
    ui: &mut egui::Ui,
    entries: &[SaveEntry],
    selected: &mut Option<usize>,
    screenshot: Option<&egui::TextureHandle>,
    portraits: &[egui::TextureHandle],
    error: Option<&str>,
) -> Option<DialogAction> {
    let mut action = None;

    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(456.0);
            save_table(ui, entries, selected);
        });
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.set_width(184.0);
            preview(ui, screenshot, portraits);
        });
    });

    if let Some(err) = error {
        ui.add_space(6.0);
        ui.colored_label(egui::Color32::from_rgb(0xE5, 0x4A, 0x4A), err);
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .add(Button::danger("Delete").disabled(selected.is_none()))
            .clicked()
        {
            action = Some(DialogAction::Delete);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(Button::primary("Load").disabled(selected.is_none()))
                .clicked()
            {
                action = Some(DialogAction::Load);
            }
            ui.add_space(8.0);
            if ui.add(Button::secondary("Cancel")).clicked() {
                action = Some(DialogAction::Cancel);
            }
        });
    });

    action
}

/// The two-column selectable list (Game Name / Time Saved).
fn save_table(ui: &mut egui::Ui, entries: &[SaveEntry], selected: &mut Option<usize>) {
    if entries.is_empty() {
        ui.add(Label::new("No saved games found.").tone(LabelTone::Muted));
        return;
    }
    let row_h = ui.text_style_height(&egui::TextStyle::Body) + 6.0;
    TableBuilder::new(ui)
        .striped(true)
        .sense(egui::Sense::click())
        .max_scroll_height(340.0)
        .column(
            Column::initial(170.0)
                .at_least(120.0)
                .clip(true)
                .resizable(true),
        )
        .column(Column::remainder().clip(true))
        .header(row_h, |mut header| {
            header.col(|ui| {
                ui.add(Label::new("Game Name").strong());
            });
            header.col(|ui| {
                ui.add(Label::new("Time Saved").strong());
            });
        })
        .body(|body| {
            body.rows(row_h, entries.len(), |mut row| {
                let i = row.index();
                row.set_selected(*selected == Some(i));
                let entry = &entries[i];
                row.col(|ui| {
                    ui.add(Label::new(display_name(&entry.save.name)));
                });
                row.col(|ui| {
                    ui.add(Label::new(&entry.time_label));
                });
                if row.response().clicked() {
                    *selected = Some(i);
                }
            });
        });
}

/// The selected save's screenshot over a row of party portrait thumbs.
fn preview(
    ui: &mut egui::Ui,
    screenshot: Option<&egui::TextureHandle>,
    portraits: &[egui::TextureHandle],
) {
    const PREVIEW_W: f32 = 184.0;
    match screenshot {
        Some(tex) => {
            let size = tex.size_vec2();
            let scale = if size.x > 0.0 {
                (PREVIEW_W / size.x).min(1.5)
            } else {
                1.0
            };
            ui.add(egui::Image::new(tex).fit_to_exact_size(size * scale));
        }
        None => {
            ui.allocate_space(egui::vec2(PREVIEW_W, PREVIEW_W * 0.75));
        }
    }
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for tex in portraits {
            let size = tex.size_vec2();
            let scale = if size.y > 0.0 {
                (30.0 / size.y).min(1.0)
            } else {
                1.0
            };
            ui.add(egui::Image::new(tex).fit_to_exact_size(size * scale));
        }
    });
}

/// Decode a save-folder BMP (screenshot / portrait) into an egui texture.
fn load_bmp_texture(
    ctx: &egui::Context,
    ds: &DataSource,
    name: &str,
) -> Option<egui::TextureHandle> {
    let bmp = BmpImporter { name }.import(ds).ok()?;
    let buffer = ImportedImage::from_bmp(bmp).image;
    let size = [buffer.width() as usize, buffer.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, buffer.as_raw());
    Some(ctx.load_texture(name, color, egui::TextureOptions::LINEAR))
}

/// The name EEKeeper shows in the "Game Name" column: the save folder
/// name with its leading `NNNNNNNNN-` numeric prefix stripped.
fn display_name(folder_name: &str) -> &str {
    match folder_name.split_once('-') {
        Some((prefix, rest))
            if !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_digit()) =>
        {
            rest
        }
        _ => folder_name,
    }
}

/// Most recent file mtime within a save folder (the actual save time),
/// falling back to the folder's own mtime.
fn folder_mtime(folder: &Path) -> SystemTime {
    let mut latest = std::fs::metadata(folder)
        .and_then(|m| m.modified())
        .unwrap_or(UNIX_EPOCH);
    if let Ok(read_dir) = std::fs::read_dir(folder) {
        for entry in read_dir.flatten() {
            if let Ok(modified) = entry.metadata().and_then(|m| m.modified())
                && modified > latest
            {
                latest = modified;
            }
        }
    }
    latest
}

/// Format a timestamp like EEKeeper's column, in the local timezone:
/// "Thursday, March 26, 2026 8:53:32 PM".
fn format_time(time: SystemTime) -> String {
    match jiff::Timestamp::try_from(time) {
        Ok(ts) => ts
            .to_zoned(jiff::tz::TimeZone::system())
            .strftime("%A, %B %-d, %Y %-I:%M:%S %p")
            .to_string(),
        Err(_) => String::new(),
    }
}
