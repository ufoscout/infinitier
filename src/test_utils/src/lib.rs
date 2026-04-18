use std::sync::OnceLock;

use image::{DynamicImage, GenericImageView};

pub mod constants;

/// Returns the path to the root of the workspace
pub fn get_root_path() -> std::path::PathBuf {
    // Search for the workspace root by walking up the tree until we find Cargo.lock
    static METADATA_LOCK: OnceLock<std::path::PathBuf> = OnceLock::new();
        METADATA_LOCK.get_or_init(|| {
            let mut path = std::env::current_dir().unwrap();
        while !path.join("Cargo.lock").exists() {
            path = path.parent().unwrap().to_path_buf();
        };
    path
    }).clone()
}

/// Returns the path to the target directory
pub fn get_target_path() -> std::path::PathBuf {
    get_root_path().join("target")
}

/// Returns the assets path
pub fn get_assets_path() -> std::path::PathBuf {
    get_root_path().join("assets")
}

/// Asserts that two images are equal
pub fn assert_images_are_equal(img_a: &DynamicImage, img_b: &DynamicImage) {
    if img_a.dimensions() != img_b.dimensions() {
        panic!("Images dimensions are different");
    }

    if img_a.to_rgba8() != img_b.to_rgba8() {
        panic!("Images bytes are different");
    }
}

/// Returns all files in a folder with a specific extension
pub fn get_all_in_folder_by_extension(
    folder: impl AsRef<std::path::Path>,
    extension: &str,
) -> Vec<std::path::PathBuf> {
    std::fs::read_dir(folder)
        .expect("Folder not found")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case(extension))
        })
        .collect()
}

/// Parse a json from a file
pub fn parse_json_file<T: serde::de::DeserializeOwned>(path: impl AsRef<std::path::Path>) -> T {
    let file = std::fs::read_to_string(path).expect("Cannot read file");
    serde_json::from_str(&file).expect("Cannot parse json")
}


#[cfg(test)]
mod tests {
    use crate::constants::BG_RESOURCES_DIR;

    use super::*;

    #[test]
    fn test_get_all_in_folder_by_extension() {
        let root = get_root_path();
        assert!(root.is_dir());
        assert!(root.join("Cargo.lock").is_file());
    }

    #[test]
    fn test_parse_assets_folder_exists() {
        let assets_path = get_assets_path().join(BG_RESOURCES_DIR);
        assert!(assets_path.is_dir());
    }
}