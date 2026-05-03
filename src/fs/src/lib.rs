#![doc = include_str!("../readme.md")]

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

/// A file system that is case insensitive
#[derive(Debug, Clone)]
pub struct CaseInsensitiveFS {
    /// The root directory
    root: PathBuf,
    /// Resource fallback folders
    fallbacks: Vec<String>,
    paths: Arc<BTreeMap<String, PathBuf>>,
}

impl CaseInsensitiveFS {
    /// Creates a new `CaseInsensitiveFS` from the given root path.
    ///
    /// The given root path is used as the root directory for the file system.
    /// All files and directories underneath the given root path are then
    /// traversed recursively, and their paths are stored in a map
    /// where the keys are the lowercased path strings and the values are the
    /// corresponding absolute paths.
    pub fn new<P: AsRef<Path>>(root: P) -> io::Result<CaseInsensitiveFS> {
        Self::new_with_fallback(root, vec![])
    }

    /// Creates a new `CaseInsensitiveFS` from the given root path.
    ///
    /// The given root path is used as the root directory for the file system.
    /// All files and directories underneath the given root path are then
    /// traversed recursively, and their paths are stored in a map
    /// where the keys are the lowercased path strings and the values are the
    /// corresponding absolute paths.
    ///
    /// The fallbacks are used to search for files that are not found in the root directory.
    pub fn new_with_fallback<P: AsRef<Path>>(
        root: P,
        fallbacks: Vec<String>,
    ) -> io::Result<CaseInsensitiveFS> {
        let root = root.as_ref().canonicalize()?;
        let paths = Arc::new(list_real_entries_recursive(&root)?);
        Ok(CaseInsensitiveFS {
            root,
            fallbacks,
            paths,
        })
    }

    /// Returns the root directory of the file system
    pub fn get_root(&self) -> &Path {
        &self.root
    }

    /// Returns the absolute path of the file or directory with the given path relative to root.
    /// The path is matched case insensitively
    pub fn get_path_opt(&self, path: &CaseInsensitivePath) -> Option<PathBuf> {
        self.paths.get(path.as_str()).cloned()
    }

    /// Tries to get the absolute path of the file or directory with the given path relative to root.
    /// The path is matched case insensitively. If the path is not found, an `io::Error` is returned.
    pub fn get_path(&self, path: &CaseInsensitivePath) -> io::Result<PathBuf> {
        match self.get_path_opt(path) {
            Some(path) => Ok(path),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path.path),
            )),
        }
    }

    /// Searches for a path in the root directory, if it does not exists, it search in a set of predefined folders
    pub fn search_path_opt(&self, path: &CaseInsensitivePath) -> Option<PathBuf> {
        if let Some(path) = self.get_path_opt(path) {
            return Some(path);
        }

        for dir in self.fallbacks.iter() {
            let search_name = format!("{}/{}", dir, path.path);
            if let Some(path) = self.paths.get(&search_name) {
                return Some(path.to_owned());
            }
        }
        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// A path that is case insensitive
pub struct CaseInsensitivePath {
    path: String,
}

impl CaseInsensitivePath {
    /// Creates a new `CaseInsensitivePath` from the given path
    pub fn new(path: &str) -> Self {
        let mut path = path
            .trim()
            .to_lowercase()
            .replace("\\", "/")
            .replace(":", "/");
        while path.starts_with("/") {
            path = path[1..].to_string();
        }
        CaseInsensitivePath { path }
    }

    /// Returns the path as a string
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Returns the base name of the path
    pub fn base_name(&self) -> &str {
        self.path
            .split('/')
            .next_back()
            .expect("Should always exists")
    }
}

/// Reads a directory and returns a map of all the files in it
/// recursively and their absolute path lowercased
fn list_real_entries_recursive(path: &Path) -> io::Result<BTreeMap<String, PathBuf>> {
    let path = path.canonicalize()?;
    let mut results = BTreeMap::new();
    recurse(&path, &path, &mut results)?;
    Ok(results)
}

fn recurse(root: &Path, path: &Path, results: &mut BTreeMap<String, PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        // Canonicalize to resolve symlinks; skip broken symlinks gracefully.
        let entry_path = match entry.path().canonicalize() {
            Ok(p) => p,
            Err(_) => continue,
        };
        // Skip entries that resolve outside the root (e.g. Wine symlinks to system paths).
        let relative_path = match entry_path.strip_prefix(root) {
            Ok(rel) => match rel.to_str() {
                Some(s) => s.to_lowercase(),
                None => continue,
            },
            Err(_) => continue,
        };

        let metadata = entry.metadata()?;
        if metadata.is_file() {
            results.insert(relative_path, entry_path);
        } else if metadata.is_dir() {
            recurse(root, &entry_path, results)?;
            results.insert(relative_path, entry_path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use infinitier_test_utils::{
        constants::{BG_RESOURCES_DIR, IWD_RESOURCES_DIR},
        get_assets_path,
    };

    use super::*;

    #[test]
    fn test_list_real_entries_recursive() {
        let current_path = std::env::current_dir().unwrap();
        let results = list_real_entries_recursive(&current_path).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_case_insensitive_fs() {
        let current_path = std::env::current_dir()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        let fs = CaseInsensitiveFS::new(current_path).unwrap();
        assert!(
            fs.get_path_opt(&CaseInsensitivePath::new("cargo.toml"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CaseInsensitivePath::new("Cargo.TOML"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CaseInsensitivePath::new("/cargo.TOML"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CaseInsensitivePath::new("/src/core/cargo.TOML"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CaseInsensitivePath::new("/Target"))
                .is_some()
        );

        assert!(
            fs.get_path(&CaseInsensitivePath::new("/src/core/cargo.TOML"))
                .is_ok()
        );
        assert!(fs.get_path(&CaseInsensitivePath::new("/Targets")).is_err());
    }

    #[test]
    fn test_search_path_in_exact_path() {
        let fs = CaseInsensitiveFS::new(get_assets_path().join(BG_RESOURCES_DIR)).unwrap();

        let path = fs.search_path_opt(&CaseInsensitivePath::new("/chitin.key"));

        assert_eq!(
            path,
            Some(
                get_assets_path()
                    .join(BG_RESOURCES_DIR)
                    .join("Chitin.key")
                    .canonicalize()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_search_path_in_fallbacks() {
        let fs = CaseInsensitiveFS::new_with_fallback(
            get_assets_path().join(IWD_RESOURCES_DIR),
            vec!["cd1".to_string(), "cd2".to_string()],
        )
        .unwrap();

        let path = fs.search_path_opt(&CaseInsensitivePath::new("/DATA/AR3603.cbf"));
        assert_eq!(
            path,
            Some(
                get_assets_path()
                    .join(IWD_RESOURCES_DIR)
                    .join("CD2/Data/AR3603.cbf")
                    .canonicalize()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_basename() {
        assert_eq!(
            CaseInsensitivePath::new("/data/AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(
            CaseInsensitivePath::new("/data/target").base_name(),
            "target"
        );
        assert_eq!(
            CaseInsensitivePath::new("data/AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(
            CaseInsensitivePath::new("data/target").base_name(),
            "target"
        );
        assert_eq!(
            CaseInsensitivePath::new("/AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(CaseInsensitivePath::new("/target").base_name(), "target");
        assert_eq!(
            CaseInsensitivePath::new("AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(CaseInsensitivePath::new("target").base_name(), "target");
        assert_eq!(CaseInsensitivePath::new("").base_name(), "");
        assert_eq!(CaseInsensitivePath::new("/").base_name(), "");
    }
}
