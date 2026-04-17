use std::path::PathBuf;

use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};

const BG_RESOURCES_DIR: &str = "../../assets/bg";
const IWD_RESOURCES_DIR: &str = "../../assets/iwd";

#[test]
fn test_search_path_in_exact_path() {
    let fs = CaseInsensitiveFS::new(BG_RESOURCES_DIR).unwrap();

    let path = fs.search_path_opt(&CaseInsensitivePath::new("/chitin.key"));

    assert_eq!(
        path,
        Some(
            PathBuf::from(BG_RESOURCES_DIR)
                .join("Chitin.key")
                .canonicalize()
                .unwrap()
        )
    );
}

#[test]
fn test_search_path_in_subfolder() {
    let fs = CaseInsensitiveFS::new(IWD_RESOURCES_DIR).unwrap();

    let path = fs.search_path_opt(&CaseInsensitivePath::new("/DATA/AR3603.cbf"));
    assert_eq!(
        path,
        Some(
            PathBuf::from(IWD_RESOURCES_DIR)
                .join("CD2/Data/AR3603.cbf")
                .canonicalize()
                .unwrap()
        )
    );
}
