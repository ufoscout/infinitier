use image::{DynamicImage, GenericImageView};

use crate::test_utils::RESOURCES_DIR;

/// Asserts that two images are equal
pub fn assert_images_are_equal(img_a: &DynamicImage, img_b: &DynamicImage) {
    if img_a.dimensions() != img_b.dimensions() {
        panic!("Images dimensions are different");
    }

    if img_a.to_rgba8() != img_b.to_rgba8() {
        panic!("Images bytes are different");
    }
}

/// Returns a path relative to the resources directory
pub fn get_path(path: impl AsRef<std::path::Path>) -> std::path::PathBuf {
    std::path::Path::new(RESOURCES_DIR).join(path)
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
