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
        let active = state.active();
        let parent = active
            .save_folder_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        self.folder_name = suggest_save_name(&active.save_name, &parent);
        self.open = true;
    }

    /// Paint the dialog if open. Reads `state.save_folder_path` +
    /// `state.save` to drive the actual export; on success it adopts
    /// the new folder as the keeper's active save (so the next click
    /// suggests `<new> (Edited NNNN)` and the title bar updates).
    pub fn show(&mut self, ctx: &egui::Context, state: &mut AppState) {
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
/// piece of `SaveAction` rather than `&mut self`. On success it
/// swaps `state.save_name` / `state.save_folder_path` to the new
/// folder and pushes a fresh window title via the viewport so the
/// next Save click anchors against the just-written save.
fn attempt_export(
    open: &mut bool,
    folder_name: &str,
    error: &mut Option<String>,
    state: &mut AppState,
) {
    let name = folder_name.trim();
    if name.is_empty() {
        *error = Some("Folder name can't be empty.".into());
        return;
    }
    let active = state.active();
    let Some(parent) = active.save_folder_path.parent() else {
        *error = Some("Save folder has no parent directory.".into());
        return;
    };
    // `parent` borrows `active.save_folder_path` — clone it so we can
    // re-borrow `state` mutably below to swap the active save.
    let parent = parent.to_path_buf();
    match perform_save_export(name, &active.save_folder_path, &parent, &active.save) {
        Ok(dest) => {
            info!("[save] wrote {}", dest.display());
            let active = state.active_mut();
            active.save_name = name.to_string();
            active.save_folder_path = dest;
            // Tab title in the header strip reflects `save_name`, so
            // the next frame already shows the new label. The window
            // title doesn't carry the save name anymore, so there's
            // no viewport title push to do here.
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

/// Suggest the next free `<base> (Edited NNNN)` folder name in `parent`.
/// Delegates to the shared `next_edited_save_name`, which increments an
/// existing `(Edited NNNN)` suffix instead of nesting another one.
pub fn suggest_save_name(current: &str, parent: &Path) -> String {
    next_edited_save_name(current, |name| parent.join(name).exists())
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

/// Maximum `(Edited NNNN)` slots probed by [`next_edited_save_name`].
const MAX_EDITED_SLOT: u32 = 9999;

/// Suggest the next free `<base> (Edited NNNN)` save name derived from
/// `current`, using `exists(name)` to skip names already taken.
///
/// If `current` already ends in a ` (Edited NNNN)` suffix, that suffix is
/// stripped to recover the base name and the search starts just past its
/// number — so re-saving `000000000-Auto-Save (Edited 0002)` yields
/// `000000000-Auto-Save (Edited 0003)`, not
/// `000000000-Auto-Save (Edited 0002) (Edited 0001)`. With no such suffix
/// the search starts at `0001`.
pub fn next_edited_save_name(current: &str, exists: impl Fn(&str) -> bool) -> String {
    let (base, existing) = split_edited_suffix(current);
    let start = existing.map(|n| n.saturating_add(1)).unwrap_or(1);
    for n in start..=MAX_EDITED_SLOT {
        let candidate = format!("{base} (Edited {n:04})");
        if !exists(&candidate) {
            return candidate;
        }
    }
    format!("{base} (Edited)")
}

/// Split a trailing ` (Edited <N>)` suffix off `name`, returning the base
/// name and the parsed number. `(name, None)` when there's no such suffix
/// (or its number doesn't parse).
fn split_edited_suffix(name: &str) -> (&str, Option<u32>) {
    const PREFIX: &str = " (Edited ";
    let Some(inner) = name.strip_suffix(')') else {
        return (name, None);
    };
    // `rfind` so the *last* suffix wins if the base also contains one.
    let Some(idx) = inner.rfind(PREFIX) else {
        return (name, None);
    };
    let digits = &inner[idx + PREFIX.len()..];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return (name, None);
    }
    match digits.parse::<u32>() {
        Ok(n) => (&inner[..idx], Some(n)),
        Err(_) => (name, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggest_save_name_picks_first_free_slot() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Quick (Edited 0001)")).unwrap();
        std::fs::create_dir(dir.path().join("Quick (Edited 0002)")).unwrap();
        assert_eq!(
            suggest_save_name("Quick", dir.path()),
            "Quick (Edited 0003)"
        );
    }

    #[test]
    fn suggest_save_name_starts_at_one_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            suggest_save_name("Quick", dir.path()),
            "Quick (Edited 0001)"
        );
    }

    #[test]
    fn suggest_save_name_for_already_edited() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            suggest_save_name("Quick (Edited 0001)", dir.path()),
            "Quick (Edited 0002)"
        );
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
        let dest = perform_save_copy("My Save (Edited 0001)", src.path(), parent.path()).unwrap();
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
    fn split_edited_suffix_recognises_and_rejects() {
        assert_eq!(split_edited_suffix("Foo (Edited 0002)"), ("Foo", Some(2)));
        assert_eq!(split_edited_suffix("Quick-Save"), ("Quick-Save", None));
        // Empty / non-numeric digit field → not a suffix.
        assert_eq!(
            split_edited_suffix("Foo (Edited )"),
            ("Foo (Edited )", None)
        );
        assert_eq!(
            split_edited_suffix("Foo (Edited abc)"),
            ("Foo (Edited abc)", None)
        );
        // A base that itself contains an (Edited) — the last one is the suffix.
        assert_eq!(
            split_edited_suffix("A (Edited 0001) (Edited 0002)"),
            ("A (Edited 0001)", Some(2))
        );
    }

    #[test]
    fn next_edited_appends_when_no_suffix() {
        assert_eq!(
            next_edited_save_name("Quick-Save", |_| false),
            "Quick-Save (Edited 0001)"
        );
    }

    #[test]
    fn next_edited_skips_taken_slots() {
        let taken: std::collections::HashSet<&str> =
            ["Quick-Save (Edited 0001)", "Quick-Save (Edited 0002)"]
                .into_iter()
                .collect();
        assert_eq!(
            next_edited_save_name("Quick-Save", |n| taken.contains(n)),
            "Quick-Save (Edited 0003)"
        );
    }

    #[test]
    fn next_edited_increments_existing_suffix_instead_of_nesting() {
        // The reported bug: current name already carries `(Edited 0002)`.
        let existing: std::collections::HashSet<&str> = [
            "000000000-Auto-Save (Edited 0001)",
            "000000000-Auto-Save (Edited 0002)",
        ]
        .into_iter()
        .collect();
        assert_eq!(
            next_edited_save_name("000000000-Auto-Save (Edited 0002)", |n| existing
                .contains(n)),
            "000000000-Auto-Save (Edited 0003)"
        );
    }

    #[test]
    fn next_edited_starts_past_current_number() {
        // From `(Edited 0005)` the next is `0006`, even when lower slots are free.
        assert_eq!(
            next_edited_save_name("Quick-Save (Edited 0005)", |_| false),
            "Quick-Save (Edited 0006)"
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
