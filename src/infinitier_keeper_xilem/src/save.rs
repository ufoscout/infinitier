//! Save action — copies the current save folder to a fresh sibling
//! `<name> (Edited NNNN)/` and re-exports the (edited) GAM so the
//! in-memory ability edits land on disk. Ported from the egui keeper's
//! `save_action`, minus the modal dialog: [`save_active`] runs the whole
//! flow in one call from the header's Save button.

use std::io;
use std::path::{Path, PathBuf};

use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::GamExporter;
use log::info;

use crate::state::AppState;

/// File extensions a save folder ships. Case-insensitive on disk.
const SAVE_FILE_EXTS: &[&str] = &["sav", "gam", "bmp", "wmp"];

/// Save the active tab to a new sibling folder and adopt it as the
/// current save (so the tab label updates and the next save anchors off
/// it). Returns the new folder path.
pub fn save_active(state: &mut AppState) -> io::Result<PathBuf> {
    let (src, save_name) = {
        let active = state.active();
        (active.save_folder_path.clone(), active.save_name.clone())
    };
    let parent = src.parent().map(Path::to_path_buf).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "save folder has no parent directory")
    })?;
    let name = suggest_save_name(&save_name, &parent);
    let dest = perform_save_export(&name, &src, &parent, &state.active().save)?;

    let active = state.active_mut();
    active.save_name = name;
    active.save_folder_path = dest.clone();
    Ok(dest)
}

/// Suggest the next free `<base> (Edited NNNN)` folder name in `parent`.
/// Delegates to the shared `next_edited_save_name`, which increments an
/// existing `(Edited NNNN)` suffix instead of nesting another one.
pub fn suggest_save_name(current: &str, parent: &Path) -> String {
    infinitier_core::save_games::next_edited_save_name(current, |name| parent.join(name).exists())
}

/// Create `parent/name/`, then copy every `.sav` / `.gam` / `.bmp` /
/// `.wmp` file from `src` (non-recursive) into it.
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
        let is_save_file = SAVE_FILE_EXTS
            .iter()
            .any(|ext| lower.ends_with(&format!(".{ext}")));
        if !is_save_file {
            continue;
        }
        let from = entry.path();
        let to = dest.join(&file_name);
        std::fs::copy(&from, &to).map_err(|e| {
            io::Error::other(format!("copy {} → {}: {e}", from.display(), to.display()))
        })?;
        copied += 1;
    }
    info!("[save] copied {copied} file(s) from {} to {}", src.display(), dest.display());
    Ok(dest)
}

/// Copy the save files via [`perform_save_copy`], then overwrite the
/// copied `.GAM` with a fresh serialisation of `gam` (the edited CRE
/// blobs live embedded inside the GAM, so re-exporting persists them).
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
    info!("[save] re-exported edited GAM → {}", gam_path.display());
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
        let dest = perform_save_copy("My Save (Edited 0001)", src.path(), parent.path()).unwrap();
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
