//! "Save" action for the keeper.
//!
//! Wires up the header's `Save` button: clicking it opens a
//! `gpui-component` confirmation dialog with a single-line text
//! input pre-filled with a free `<current> (Edited NNNN)` slot. On
//! confirm, every save-game file (`.sav`, `.gam`, `.bmp`, `.wmp` —
//! case-insensitive) in the source folder is copied into a fresh
//! sibling folder.
//!
//! The keeper can't *write* save data yet — the GAM mutation surface
//! isn't there — but the export step is still useful: it gives users
//! a stable backup folder they can drop their hand-edited files into
//! before this lands.

use std::io;
use std::path::{Path, PathBuf};

use gpui::{AppContext as _, Context, Entity, ParentElement, SharedString, Styled, Window};
use gpui_component::WindowExt as _;
use gpui_component::button::{Button, ButtonVariant, ButtonVariants as _};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::input::{Input, InputState};
use gpui_component::{Icon, IconName, Sizable as _, v_flex};
use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GamExporter;
use log::{error, info};

use crate::app::KeeperApp;

/// File extensions a save folder ships. Case-insensitive on disk.
const SAVE_FILE_EXTS: &[&str] = &["sav", "gam", "bmp", "wmp"];

/// Maximum number of `(Edited NNNN)` slots the suggester will probe
/// before giving up. 9999 is way past the point any real workflow
/// hits, but the bound stops a pathological folder from looping
/// forever.
const MAX_EDITED_SLOT: u32 = 9999;

/// "Save" button rendered at the right edge of the header. Click
/// opens the confirmation dialog.
pub fn render_save_button(cx: &mut Context<KeeperApp>) -> Button {
    Button::new("keeper-save")
        .icon(Icon::new(IconName::Inbox))
        .label("Save")
        .with_variant(ButtonVariant::Primary)
        .small()
        .on_click(cx.listener(|this, _ev, window, cx| {
            open_save_dialog(this, window, cx);
        }))
}

/// Build & present the confirmation dialog. Captures the current
/// save's source folder + parent so the on-ok handler can run the
/// copy without touching `KeeperApp` mutably (it doesn't need to —
/// the keeper's in-memory state doesn't change here).
fn open_save_dialog(this: &KeeperApp, window: &mut Window, cx: &mut Context<KeeperApp>) {
    let active = this.state.active();
    let src = active.save_folder_path.clone();
    let parent = match src.parent() {
        Some(p) => p.to_path_buf(),
        None => {
            error!(
                "[save] cannot save: source folder {} has no parent",
                src.display()
            );
            return;
        }
    };
    let suggested = suggest_save_name(&active.save_name, &parent);
    // Captured into the on-ok closure so it can read the *current*
    // `ImportedGam` at click time — by then the user may have edited
    // ability scores or anything else, and we need the latest state.
    let view: Entity<KeeperApp> = cx.entity();

    // One InputState per dialog opening. Cloned cheaply into both
    // the body (`Input::new`) and the on-ok closure (`.read(cx)`).
    let input: Entity<InputState> = cx.new(|cx| {
        InputState::new(window, cx).default_value(SharedString::from(suggested.clone()))
    });
    let input_for_close: Entity<InputState> = input.clone();
    let src_for_close = src.clone();
    let parent_for_close = parent.clone();
    let view_for_close = view.clone();

    window.open_dialog(cx, move |dialog, _window, _cx| {
        let body_input = input.clone();
        let src_for_ok = src_for_close.clone();
        let parent_for_ok = parent_for_close.clone();
        let input_for_ok = input_for_close.clone();
        let view_for_ok = view_for_close.clone();
        dialog
            .title("Save edited save game")
            .child(
                v_flex()
                    .gap_2()
                    .child(SharedString::from(
                        "Save the current save game (GAM, SAV, BMP, WMP) into a new sibling folder.",
                    ))
                    .child(SharedString::from("Folder name:"))
                    .child(Input::new(&body_input)),
            )
            .confirm()
            .button_props(
                DialogButtonProps::default()
                    .ok_text("Save")
                    .cancel_text("Cancel"),
            )
            .on_ok(move |_event, _window, cx| {
                let name = input_for_ok.read(cx).value().to_string();
                let name = name.trim();
                if name.is_empty() {
                    error!("[save] empty folder name; aborting");
                    // Keep the dialog open so the user can fix it.
                    return false;
                }
                // Snapshot the latest in-memory GAM (with whatever
                // ability edits the user has made) for re-export.
                let gam_snapshot = view_for_ok.read(cx).state.active().imported_gam.clone();
                match perform_save_export(name, &src_for_ok, &parent_for_ok, &gam_snapshot) {
                    Ok(dest) => {
                        info!("[save] wrote {}", dest.display());
                        // Adopt the new folder as the active save so
                        // the next Save click anchors against it and
                        // the save-tab strip's label refreshes. The
                        // window title doesn't carry the save name
                        // anymore, so no `set_window_title` push here.
                        view_for_ok.update(cx, |this, cx| {
                            let active = this.state.active_mut();
                            active.save_name = name.to_string();
                            active.save_folder_path = dest.clone();
                            cx.notify();
                        });
                        true
                    }
                    Err(e) => {
                        error!(
                            "[save] failed to write '{}' into {}: {e}",
                            name,
                            parent_for_ok.display(),
                        );
                        // Same posture: leave the dialog open so the
                        // user can adjust the name (e.g. clash with
                        // an existing folder).
                        false
                    }
                }
            })
    });
}

/// Find the first free `<current> (Edited NNNN)` slot in `parent`.
/// Falls back to `<current> (Edited)` when every numbered slot up to
/// [`MAX_EDITED_SLOT`] is taken — that's strictly defensive; no real
/// folder will ever get there.
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
///
/// Refuses to overwrite — if the destination already exists, an
/// `AlreadyExists` error bubbles up so the user can pick another
/// name (the dialog stays open in that case).
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
/// in-memory ability-score edits the user has made are stored as
/// embedded CRE blobs *inside* the GAM, so re-exporting through
/// `GamExporter` is what makes them persist.
///
/// The SAV / BMP / WMP files are copied verbatim — none of them
/// carry data the keeper currently edits.
pub fn perform_save_export(
    name: &str,
    src: &Path,
    parent: &Path,
    gam: &ImportedGam,
) -> io::Result<PathBuf> {
    let dest = perform_save_copy(name, src, parent)?;

    // Find the copied `.GAM`. The on-disk filename is engine-specific
    // (BALDUR.GAM / ICEWIND.GAM / ICEWIND2.GAM / TORMENT.GAM); using
    // the file we just copied means we don't have to encode that
    // mapping here.
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
                format!(
                    "no .GAM file in copied save folder {}",
                    dest.display(),
                ),
            )
        })?;

    // `ImportedGam::export` consumes self — clone first so the
    // keeper keeps its working copy.
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
        // 0003 should be free.
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
        // Mixed-case extensions on purpose.
        std::fs::write(src.path().join("BALDUR.GAM"), b"gam").unwrap();
        std::fs::write(src.path().join("AR0072.sav"), b"sav").unwrap();
        std::fs::write(src.path().join("PORTRT0.bmp"), b"bmp").unwrap();
        std::fs::write(src.path().join("WORLDMAP.WMP"), b"wmp").unwrap();
        std::fs::write(src.path().join("note.txt"), b"ignored").unwrap();

        let dest =
            perform_save_copy("My Save (Edited 0001)", src.path(), parent.path()).unwrap();
        assert!(dest.is_dir());
        // The four save files made it across, the txt didn't.
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
