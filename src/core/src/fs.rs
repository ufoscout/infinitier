use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A file system that is case insensitive
pub struct CaseInsensitiveFS {
    root: PathBuf,
    paths: BTreeMap<String, PathBuf>,
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
        let root = root.as_ref().canonicalize()?;
        let paths = list_real_entries_recursive(&root)?;
        // println!("paths: \n{:#?}", paths);
        Ok(CaseInsensitiveFS { root, paths })
    }

    /// Returns the root directory of the file system
    pub fn get_root(&self) -> &Path {
        &self.root
    }

    /// Returns the absolute path of the file or directory with the given path relative to root.
    /// The path is matched case insensitively
    pub fn get_path_opt(&self, path: &str) -> Option<PathBuf> {
        let mut path = path.to_lowercase();
        if path.starts_with("/") {
            path = path[1..].to_string();
        }
        self.paths.get(&path).cloned()
    }

    /// Tries to get the absolute path of the file or directory with the given path relative to root.
    /// The path is matched case insensitively. If the path is not found, an `io::Error` is returned.
    pub fn get_path(&self, path: &str) -> io::Result<PathBuf> {
        match self.get_path_opt(path) {
            Some(path) => Ok(path),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path),
            )),
        }
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
        let metadata = entry.metadata()?;
        let entry_path = entry.path().canonicalize()?;
        let relative_path = entry_path
            .strip_prefix(root)
            .unwrap_or_else(|_| panic!("Cannot strip prefix from path {}", path.display()))
            .to_str()
            .unwrap_or_else(|| panic!("Cannot convert path to string {}", path.display()))
            .to_lowercase();

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
        assert!(fs.get_path_opt("cargo.toml").is_some());
        assert!(fs.get_path_opt("Cargo.TOML").is_some());
        assert!(fs.get_path_opt("/cargo.TOML").is_some());
        assert!(fs.get_path_opt("/src/core/cargo.TOML").is_some());
        assert!(fs.get_path_opt("/Target").is_some());

        assert!(fs.get_path("/src/core/cargo.TOML").is_ok());
        assert!(fs.get_path("/Targets").is_err());
    }
}
