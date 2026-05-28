use std::sync::OnceLock;

use image::{DynamicImage, GenericImageView};

pub mod constants;

/// Returns the path to the root of the workspace
pub fn get_root_path() -> std::path::PathBuf {
    // Search for the workspace root by walking up the tree until we find Cargo.lock
    static METADATA_LOCK: OnceLock<std::path::PathBuf> = OnceLock::new();
    METADATA_LOCK
        .get_or_init(|| {
            let mut path = std::env::current_dir().unwrap();
            while !path.join("Cargo.lock").exists() {
                path = path.parent().unwrap().to_path_buf();
            }
            path
        })
        .clone()
}

/// Returns the path to the target directory
pub fn get_target_path() -> std::path::PathBuf {
    get_root_path().join("target")
}

/// Returns the assets path
pub fn get_assets_path() -> std::path::PathBuf {
    get_root_path().join("assets")
}

/// Asserts that two images are equal.
///
/// `tolerance` is the max allowed absolute per-channel delta (any R/G/B/A
/// channel of any pixel). `None` means strict bytewise equality.
pub fn assert_images_are_equal(img_a: &DynamicImage, img_b: &DynamicImage, tolerance: Option<u8>) {
    if img_a.dimensions() != img_b.dimensions() {
        panic!("Images dimensions are different");
    }

    let a = img_a.to_rgba8();
    let b = img_b.to_rgba8();

    match tolerance {
        None => {
            if a != b {
                panic!("Images bytes are different");
            }
        }
        Some(t) => {
            for (i, (pa, pb)) in a.chunks_exact(4).zip(b.chunks_exact(4)).enumerate() {
                for c in 0..4 {
                    let d = pa[c].abs_diff(pb[c]);
                    if d > t {
                        panic!(
                            "Pixel {} channel {} differs by {} (a={:?}, b={:?}, tolerance={})",
                            i, c, d, pa, pb, t
                        );
                    }
                }
            }
        }
    }
}

/// Returns every regular file in `folder` whose extension matches
/// `extension` (case-insensitive). When `recursive` is `true`, the
/// whole subtree under `folder` is walked depth-first; otherwise only
/// direct children are considered. The output is sorted so iteration
/// order is stable across platforms.
///
/// Panics if `folder` doesn't exist or isn't readable — same posture
/// as before.
pub fn get_all_in_folder_by_extension(
    folder: impl AsRef<std::path::Path>,
    extension: &str,
    recursive: bool,
) -> Vec<std::path::PathBuf> {
    let mut out: Vec<std::path::PathBuf> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![folder.as_ref().to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("Folder not found: {} ({e})", dir.display()));
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let kind = match entry.file_type() {
                Ok(k) => k,
                Err(_) => continue,
            };
            if kind.is_dir() {
                if recursive {
                    stack.push(path);
                }
                continue;
            }
            if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
            {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Parse a json from a file
pub fn parse_json_file<T: serde::de::DeserializeOwned>(path: impl AsRef<std::path::Path>) -> T {
    let file = std::fs::read_to_string(path).expect("Cannot read file");
    serde_json::from_str(&file).expect("Cannot parse json")
}

/// Starts a logger on stdout
pub fn start_logger() {
    let _ = env_logger::builder()
        .parse_filters("debug")
        .format_timestamp(None)
        .try_init();
}

#[cfg(test)]
mod tests {
    use crate::constants::BG_RESOURCES_DIR;

    use super::*;

    #[test]
    fn test_get_root_path_finds_workspace() {
        let root = get_root_path();
        assert!(root.is_dir());
        assert!(root.join("Cargo.lock").is_file());
    }

    #[test]
    fn test_parse_assets_folder_exists() {
        let assets_path = get_assets_path().join("KEY").join(BG_RESOURCES_DIR.0);
        assert!(assets_path.is_dir());
    }

    /// Build the same scratch tree used by both flag-mode tests so
    /// `recursive=true` and `recursive=false` can be compared head to
    /// head on identical input.
    ///
    /// Shape:
    /// ```text
    /// root/
    /// ├── top1.txt
    /// ├── top2.TXT          # mixed case — extension match is
    /// │                     # case-insensitive
    /// ├── ignore.md         # different extension
    /// └── sub/
    ///     ├── nested.txt
    ///     └── deeper/
    ///         └── leaf.txt
    /// ```
    fn make_scratch_tree() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("top1.txt"), b"a").unwrap();
        std::fs::write(root.join("top2.TXT"), b"b").unwrap();
        std::fs::write(root.join("ignore.md"), b"c").unwrap();
        let sub = root.join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("nested.txt"), b"d").unwrap();
        let deeper = sub.join("deeper");
        std::fs::create_dir(&deeper).unwrap();
        std::fs::write(deeper.join("leaf.txt"), b"e").unwrap();
        dir
    }

    #[test]
    fn get_all_in_folder_by_extension_non_recursive_finds_top_level_only() {
        let dir = make_scratch_tree();
        let mut got = get_all_in_folder_by_extension(dir.path(), "txt", false);
        // Strip the scratch prefix so the assertion is portable.
        let names: Vec<String> = got
            .drain(..)
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            names,
            vec!["top1.txt".to_string(), "top2.TXT".to_string()],
            "should match top-level files, case-insensitively, sorted",
        );
    }

    #[test]
    fn get_all_in_folder_by_extension_recursive_descends() {
        let dir = make_scratch_tree();
        let got = get_all_in_folder_by_extension(dir.path(), "txt", true);
        // Map back to relative paths so the assertion stays portable.
        let rels: Vec<String> = got
            .iter()
            .map(|p| {
                p.strip_prefix(dir.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(
            rels,
            vec![
                "sub/deeper/leaf.txt".to_string(),
                "sub/nested.txt".to_string(),
                "top1.txt".to_string(),
                "top2.TXT".to_string(),
            ],
            "recursive walk should find every .txt under the tree, sorted",
        );
    }

    #[test]
    fn get_all_in_folder_by_extension_ignores_other_extensions() {
        let dir = make_scratch_tree();
        let got = get_all_in_folder_by_extension(dir.path(), "md", true);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].file_name().unwrap(), "ignore.md");
    }

    #[test]
    fn get_all_in_folder_by_extension_empty_folder_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let got = get_all_in_folder_by_extension(dir.path(), "txt", true);
        assert!(got.is_empty());
    }
}
