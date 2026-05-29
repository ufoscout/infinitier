//! "Save" action for the egui keeper.
//!
//! Mirrors the GPUI keeper's `save_action`: a Save button in the
//! header pops a confirmation dialog with a pre-filled `<current>
//! (Edited NNNN)` folder name. On confirm, every save-game file
//! (`.sav`, `.gam`, `.bmp`, `.wmp` — case-insensitive) is copied
//! into a fresh sibling folder and the GAM is re-exported so any
//! in-memory edits land on disk.
//!
//! The dialog itself is an [`egui_components::Dialog`] — a themed
//! modal backed by `egui::Modal` (backdrop, drop shadow, Esc / × to
//! dismiss).

use std::io;
use std::path::{Path, PathBuf};

use eframe::egui;
use egui_components::{Button, Dialog, Input, Label, LabelTone, Variant};
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GamExporter;
use log::{error, info};

use crate::state::AppState;

/// File extensions a save folder ships. Case-insensitive on disk.
const SAVE_FILE_EXTS: &[&str] = &["sav", "gam", "bmp", "wmp"];

/// Maximum number of `(Edited NNNN)` slots the suggester will probe
/// before giving up.
const MAX_EDITED_SLOT: u32 = 9999;

/// Owned state for the Save dialog. Lives on `KeeperApp` so a single
/// instance survives across frames (egui's immediate-mode flow means
/// the dialog state — text buffer, error, open flag — must be held
/// by the host).
#[derive(Default)]
pub struct SaveAction {
    open: bool,
    /// Pre-filled name; rebuilt the moment the user clicks Save so
    /// the next available `(Edited NNNN)` slot is shown.
    folder_name: String,
    /// Last-attempt error, surfaced inline above the buttons.
    error: Option<String>,
}

impl SaveAction {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the dialog, pre-filling the folder name with the next
    /// free `(Edited NNNN)` slot in the current save's parent dir.
    pub fn open(&mut self, state: &AppState) {
        self.error = None;
        let parent = state
            .save_folder_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        self.folder_name = suggest_save_name(&state.save_name, &parent);
        self.open = true;
    }

    /// Paint the dialog if open. Reads `state.save_folder_path` +
    /// `state.save` to drive the actual export; closes itself on
    /// success.
    pub fn show(&mut self, ctx: &egui::Context, state: &AppState) {
        // Split-borrow `self` so the body closure can mutate
        // `folder_name` / `error` while `Dialog::show` holds
        // `&mut open` for backdrop / Esc / × dismiss handling.
        let SaveAction {
            open,
            folder_name,
            error,
        } = self;
        if !*open {
            return;
        }
        // Action picked inside the dialog body; resolved after the
        // dialog returns so we can re-borrow `error` / `open` /
        // call helpers freely.
        let mut action: Option<DialogAction> = None;
        Dialog::new("Save edited save game")
            .width(420.0)
            .show(ctx, open, |ui| {
                ui.add(Label::new(
                    "Save the current save game (GAM, SAV, BMP, WMP) into a new sibling folder.",
                ));
                ui.add_space(6.0);
                ui.add(Label::new("Folder name:").tone(LabelTone::Muted));
                ui.add(Input::new(folder_name).width(380.0));
                if let Some(err) = error.as_deref() {
                    ui.add_space(4.0);
                    ui.colored_label(egui::Color32::from_rgb(0xE5, 0x4A, 0x4A), err);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.add(Button::secondary("Cancel")).clicked() {
                        action = Some(DialogAction::Cancel);
                    }
                    if ui
                        .add(Button::new("Save").variant(Variant::Primary))
                        .clicked()
                    {
                        action = Some(DialogAction::Save);
                    }
                });
            });
        match action {
            Some(DialogAction::Cancel) => *open = false,
            Some(DialogAction::Save) => attempt_export(open, folder_name, error, state),
            None => {}
        }
    }
}

/// Choice resolved from the dialog body. Carried back to the
/// caller via `let mut action = None;` so we can `*open` / `error`
/// after `Dialog::show` has released its `&mut open` borrow.
enum DialogAction {
    Cancel,
    Save,
}

/// Free-function form of the export attempt — invoked after the
/// dialog body returns, when we hold mutable references to every
/// piece of `SaveAction` rather than `&mut self`.
fn attempt_export(
    open: &mut bool,
    folder_name: &str,
    error: &mut Option<String>,
    state: &AppState,
) {
    let name = folder_name.trim();
    if name.is_empty() {
        *error = Some("Folder name can't be empty.".into());
        return;
    }
    let Some(parent) = state.save_folder_path.parent() else {
        *error = Some("Save folder has no parent directory.".into());
        return;
    };
    match perform_save_export(name, &state.save_folder_path, parent, &state.save) {
        Ok(dest) => {
            info!("[save] wrote {}", dest.display());
            *open = false;
            *error = None;
        }
        Err(e) => {
            error!(
                "[save] failed to write '{name}' into {}: {e}",
                parent.display(),
            );
            *error = Some(e.to_string());
        }
    }
}

/// Find the first free `<current> (Edited NNNN)` slot in `parent`.
pub fn suggest_save_name(current: &str, parent: &Path) -> String {
    for n in 1..=MAX_EDITED_SLOT {
        let candidate = format!("{current} (Edited {:04})", n);
        if !parent.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{current} (Edited)")
}

/// Create `parent/name/`, then copy every `.sav` / `.gam` / `.bmp` /
/// `.wmp` file from `src` (non-recursive) into it. Returns the new
/// folder's path on success.
pub fn perform_save_copy(name: &str, src: &Path, parent: &Path) -> io::Result<PathBuf> {
    let dest = parent.join(name);
    if dest.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("destination already exists: {}", dest.display()),
        ));
    }
    std::fs::create_dir(&dest)?;
    let mut copied = 0usize;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name();
        let lower = file_name.to_string_lossy().to_lowercase();
        let matches_save_ext = SAVE_FILE_EXTS
            .iter()
            .any(|ext| lower.ends_with(&format!(".{ext}")));
        if !matches_save_ext {
            continue;
        }
        let from = entry.path();
        let to = dest.join(&file_name);
        std::fs::copy(&from, &to).map_err(|e| {
            io::Error::other(format!("copy {} → {}: {e}", from.display(), to.display()))
        })?;
        copied += 1;
    }
    info!(
        "[save] copied {copied} file(s) from {} to {}",
        src.display(),
        dest.display(),
    );
    Ok(dest)
}

/// Full save flow: copy every save-game file from `src` into a new
/// `parent/name/` folder via [`perform_save_copy`], then overwrite
/// the copied `.GAM` file with a fresh serialisation of `gam`. The
/// in-memory ability-score edits the user made are stored as
/// embedded CRE blobs *inside* the GAM, so re-exporting through
/// `GamExporter` is what makes them persist.
pub fn perform_save_export(
    name: &str,
    src: &Path,
    parent: &Path,
    gam: &ImportedGam,
) -> io::Result<PathBuf> {
    let dest = perform_save_copy(name, src, parent)?;
    let gam_path = std::fs::read_dir(&dest)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("gam"))
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("no .GAM file in copied save folder {}", dest.display()),
            )
        })?;
    let exported = gam.clone().export()?;
    GamExporter.export_to_file(&exported, &gam_path)?;
    info!(
        "[save] re-exported edited GAM → {} ({} bytes)",
        gam_path.display(),
        std::fs::metadata(&gam_path).map(|m| m.len()).unwrap_or(0),
    );
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_save_name_picks_first_free_slot() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Quick (Edited 0001)")).unwrap();
        std::fs::create_dir(dir.path().join("Quick (Edited 0002)")).unwrap();
        assert_eq!(suggest_save_name("Quick", dir.path()), "Quick (Edited 0003)");
    }

    #[test]
    fn suggest_save_name_starts_at_one_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(suggest_save_name("Quick", dir.path()), "Quick (Edited 0001)");
    }

    #[test]
    fn perform_save_copy_writes_known_extensions_only() {
        let src = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("BALDUR.GAM"), b"gam").unwrap();
        std::fs::write(src.path().join("AR0072.sav"), b"sav").unwrap();
        std::fs::write(src.path().join("PORTRT0.bmp"), b"bmp").unwrap();
        std::fs::write(src.path().join("WORLDMAP.WMP"), b"wmp").unwrap();
        std::fs::write(src.path().join("note.txt"), b"ignored").unwrap();
        let dest =
            perform_save_copy("My Save (Edited 0001)", src.path(), parent.path()).unwrap();
        assert!(dest.is_dir());
        let mut got: Vec<String> = std::fs::read_dir(&dest)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        got.sort();
        assert_eq!(
            got,
            vec![
                "AR0072.sav".to_string(),
                "BALDUR.GAM".to_string(),
                "PORTRT0.bmp".to_string(),
                "WORLDMAP.WMP".to_string(),
            ],
        );
    }

    #[test]
    fn perform_save_copy_refuses_to_overwrite() {
        let src = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("X.SAV"), b"sav").unwrap();
        std::fs::create_dir(parent.path().join("Existing")).unwrap();
        let err = perform_save_copy("Existing", src.path(), parent.path()).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    }
}
