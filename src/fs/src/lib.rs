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
    pub fn get_path_opt(&self, path: &CiPath) -> Option<PathBuf> {
        self.paths.get(path.as_str()).cloned()
    }

    /// Tries to get the absolute path of the file or directory with the given path relative to root.
    /// The path is matched case insensitively. If the path is not found, an `io::Error` is returned.
    pub fn get_path(&self, path: &CiPath) -> io::Result<PathBuf> {
        match self.get_path_opt(path) {
            Some(path) => Ok(path),
            None => Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path.path),
            )),
        }
    }

    /// Searches for a path in the root directory, if it does not exists, it search in a set of predefined folders
    pub fn search_path_opt(&self, path: &CiPath) -> Option<PathBuf> {
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

    /// Returns a list of files in a folder, optionally filtered by extension.
    /// When `extension` is `None`, all files in scope are returned.
    /// The path is matched case insensitively. When `recursive` is
    /// false, only direct children of `path` are returned; otherwise
    /// the whole subtree under `path` is walked.
    pub fn list_files(
        &self,
        path: &CiPath,
        extension: Option<&str>,
        recursive: bool,
    ) -> Vec<PathBuf> {
        let needle = path.path.to_lowercase();
        let mut results = vec![];
        for (key, value) in self.paths.iter() {
            let in_scope = if needle.is_empty() {
                // Empty needle = root: whole tree (recursive) or
                // root-level entries only (non-recursive).
                recursive || !key.contains('/')
            } else if recursive {
                // Under `needle` directory: equal to it, or starting
                // with `{needle}/` so we don't match `needleX/...`.
                key == needle.as_str() || key.starts_with(&format!("{needle}/"))
            } else {
                // Non-recursive: direct children only — key must be
                // `{needle}/{name}` with no further `/` in `{name}`.
                key.strip_prefix(&format!("{needle}/"))
                    .is_some_and(|rest| !rest.contains('/'))
            };
            if !in_scope {
                continue;
            }
            if !value.is_file() {
                continue;
            }
            match extension {
                Some(ext_filter) => {
                    if let Some(ext) = value.extension()
                        && ext.eq_ignore_ascii_case(ext_filter)
                    {
                        results.push(value.to_owned());
                    }
                }
                None => results.push(value.to_owned()),
            }
        }
        results
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// A path that is case insensitive
pub struct CiPath {
    path: String,
}

impl CiPath {
    /// Creates a new `CiPath` from the given path
    pub fn new(path: &str) -> Self {
        let mut path = path
            .trim()
            .to_lowercase()
            .replace("\\", "/")
            .replace(":", "/");
        while path.starts_with("/") {
            path = path[1..].to_string();
        }
        CiPath { path }
    }

    /// Returns the path relative to the root
    pub fn as_str(&self) -> &str {
        &self.path
    }

    /// Returns the base name of the path.
    /// E.g.:
    /// - `foo/bar` -> `bar`
    /// - `foo/bar/` -> `bar`
    /// - `/foo/bar.exe` -> `bar.exe`
    pub fn base_name(&self) -> &str {
        self.path
            .split('/')
            .next_back()
            .expect("Should always exists")
    }

    /// Returns the extension of the path.
    /// E.g.:
    /// - `foo/bar` -> ``
    /// - `foo/bar/` -> ``
    /// - `/foo/bar.exe` -> `.exe`
    pub fn extension(&self) -> Option<&str> {
        self.path
            .split('.')
            .nth(1)
            .filter(|ext| !ext.is_empty())
    }

    /// Returns the base name of the path without the extension.
    /// E.g.:
    /// - `foo/bar` -> `bar`
    /// - `foo/bar/` -> `bar`
    /// - `/foo/bar.exe` -> `bar`
    pub fn base_name_without_extension(&self) -> &str {
        self.base_name().split('.').next().unwrap()
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
    use std::fs::File;

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
            fs.get_path_opt(&CiPath::new("cargo.toml"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CiPath::new("Cargo.TOML"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CiPath::new("/cargo.TOML"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CiPath::new("/src/core/cargo.TOML"))
                .is_some()
        );
        assert!(
            fs.get_path_opt(&CiPath::new("/Target"))
                .is_some()
        );

        assert!(
            fs.get_path(&CiPath::new("/src/core/cargo.TOML"))
                .is_ok()
        );
        assert!(fs.get_path(&CiPath::new("/Targets")).is_err());
    }

    #[test]
    fn test_search_path_in_exact_path() {
        let fs =
            CaseInsensitiveFS::new(get_assets_path().join("KEY").join(BG_RESOURCES_DIR.0)).unwrap();

        let path = fs.search_path_opt(&CiPath::new("/chitin.key"));

        assert_eq!(
            path,
            Some(
                get_assets_path()
                    .join("KEY")
                    .join(BG_RESOURCES_DIR.0)
                    .join("Chitin.key")
                    .canonicalize()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_search_path_in_fallbacks() {
        let fs = CaseInsensitiveFS::new_with_fallback(
            get_assets_path().join("KEY").join(IWD_RESOURCES_DIR.0),
            vec!["cd1".to_string(), "cd2".to_string()],
        )
        .unwrap();

        let path = fs.search_path_opt(&CiPath::new("/DATA/AR3603.cbf"));
        assert_eq!(
            path,
            Some(
                get_assets_path()
                    .join("KEY")
                    .join(IWD_RESOURCES_DIR.0)
                    .join("CD2/Data/AR3603.cbf")
                    .canonicalize()
                    .unwrap()
            )
        );
    }

    #[test]
    fn test_ci_path_new_normalizes() {
        // Lowercases
        assert_eq!(CiPath::new("Foo/Bar.EXE").as_str(), "foo/bar.exe");
        // Trims surrounding whitespace
        assert_eq!(CiPath::new("  data/file  ").as_str(), "data/file");
        // Backslashes become forward slashes
        assert_eq!(CiPath::new("data\\sub\\file").as_str(), "data/sub/file");
        // Colons become forward slashes (the `\` after `C:` is also replaced,
        // and since the colon swap happens after the backslash swap, the two
        // adjacent separators in `C:\` collapse to `//` rather than a single `/`).
        assert_eq!(CiPath::new("C:\\Windows\\file").as_str(), "c//windows/file");
        // A standalone colon without an adjacent backslash yields a single `/`.
        assert_eq!(CiPath::new("C:foo").as_str(), "c/foo");
        // Leading slashes are stripped (all of them)
        assert_eq!(CiPath::new("/data/file").as_str(), "data/file");
        assert_eq!(CiPath::new("///data/file").as_str(), "data/file");
        // Empty input stays empty
        assert_eq!(CiPath::new("").as_str(), "");
        assert_eq!(CiPath::new("   ").as_str(), "");
        // Bare slash collapses to empty
        assert_eq!(CiPath::new("/").as_str(), "");
        // Combined: trim + lowercase + backslash + leading slash
        assert_eq!(CiPath::new("  \\Foo\\Bar  ").as_str(), "foo/bar");
    }

    #[test]
    fn test_ci_path_as_str() {
        let p = CiPath::new("/Data/AR3603.CBF");
        assert_eq!(p.as_str(), "data/ar3603.cbf");

        // Round-trip: as_str returns the same value used by base_name etc.
        let p = CiPath::new("foo/bar");
        assert_eq!(p.as_str(), "foo/bar");
    }

    #[test]
    fn test_ci_path_extension() {
        assert_eq!(CiPath::new("/foo/bar.exe").extension(), Some("exe"));
        assert_eq!(CiPath::new("file.JSON").extension(), Some("json"));
        // Empty extension after the dot returns None
        assert_eq!(CiPath::new("file.").extension(), None);
        // No dot at all
        assert_eq!(CiPath::new("foo/bar").extension(), None);
        assert_eq!(CiPath::new("target").extension(), None);
        // Trailing slash without dot
        assert_eq!(CiPath::new("foo/bar/").extension(), None);
        // Empty path
        assert_eq!(CiPath::new("").extension(), None);
    }

    #[test]
    fn test_ci_path_base_name_without_extension() {
        assert_eq!(
            CiPath::new("/foo/bar.exe").base_name_without_extension(),
            "bar"
        );
        assert_eq!(
            CiPath::new("/data/AR3603.cbf").base_name_without_extension(),
            "ar3603"
        );
        // No extension
        assert_eq!(
            CiPath::new("/foo/target").base_name_without_extension(),
            "target"
        );
        assert_eq!(CiPath::new("target").base_name_without_extension(), "target");
        // Trailing slash: base_name is "", so stem is ""
        assert_eq!(CiPath::new("foo/bar/").base_name_without_extension(), "");
        // Empty path
        assert_eq!(CiPath::new("").base_name_without_extension(), "");
        assert_eq!(CiPath::new("/").base_name_without_extension(), "");
        // Multi-dot file: stem is everything before the first dot in the base_name
        assert_eq!(
            CiPath::new("/foo/archive.tar.gz").base_name_without_extension(),
            "archive"
        );
    }

    #[test]
    fn test_basename() {
        assert_eq!(
            CiPath::new("/data/AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(
            CiPath::new("/data/target").base_name(),
            "target"
        );
        assert_eq!(
            CiPath::new("data/AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(
            CiPath::new("data/target").base_name(),
            "target"
        );
        assert_eq!(
            CiPath::new("/AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(CiPath::new("/target").base_name(), "target");
        assert_eq!(
            CiPath::new("AR3603.cbf").base_name(),
            "ar3603.cbf"
        );
        assert_eq!(CiPath::new("target").base_name(), "target");
        assert_eq!(CiPath::new("").base_name(), "");
        assert_eq!(CiPath::new("/").base_name(), "");
    }

    #[test]
    fn test_search_files_by_extension() {
        // Arrange
        // Create a temporary directory structure:
        // - root/
        //   - file1.json
        //   - file2.json
        //   - file1.INI
        //   - ini/
        //     - file1.ini
        //     - file1.Json
        //   - INNER/
        //     - inner/
        //       - file1.ini
        //     - file1.json
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        {
            // root files
            File::create(root.join("file1.json")).unwrap();
            File::create(root.join("file2.json")).unwrap();
            File::create(root.join("file1.INI")).unwrap();

            // ini/
            let ini_dir = root.join("ini");
            fs::create_dir(&ini_dir).unwrap();
            File::create(ini_dir.join("file1.ini")).unwrap();
            File::create(ini_dir.join("file1.Json")).unwrap();

            // inner/
            let inner_dir = root.join("INNER");
            fs::create_dir(&inner_dir).unwrap();

            // inner/inner/
            let inner_inner_dir = inner_dir.join("inner");
            fs::create_dir(&inner_inner_dir).unwrap();
            File::create(inner_inner_dir.join("file1.ini")).unwrap();

            // inner/file1.json
            File::create(inner_dir.join("file1.json")).unwrap();
        }

        let fs = CaseInsensitiveFS::new(root).unwrap();

        // Act - recursive - 1
        {
            let files = fs.list_files(
                &CiPath {
                    path: "".to_owned(),
                },
                Some("json"),
                true,
            );

            // Assert
            assert_eq!(files.len(), 4);
            assert!(files.contains(&root.join("file1.json")));
            assert!(files.contains(&root.join("file2.json")));
            assert!(files.contains(&root.join("ini/file1.Json")));
            assert!(files.contains(&root.join("INNER/file1.json")));
        }

        // Act - recursive - 2
        {
            let files = fs.list_files(
                &CiPath {
                    path: "INNER".to_owned(),
                },
                Some("json"),
                true,
            );

            // Assert
            assert_eq!(files.len(), 1);
            assert!(files.contains(&root.join("INNER/file1.json")));
        }

        // Act - recursive - 3
        {
            let files = fs.list_files(
                &CiPath {
                    path: "INNER".to_owned(),
                },
                Some("ini"),
                true,
            );

            // Assert
            assert_eq!(files.len(), 1);
            assert!(files.contains(&root.join("INNER/inner/file1.ini")));
        }

        // Act - recursive - 4
        {
            let files = fs.list_files(
                &CiPath {
                    path: "INNER/inner".to_owned(),
                },
                Some("ini"),
                true,
            );

            // Assert
            assert_eq!(files.len(), 1);
            assert!(files.contains(&root.join("INNER/inner/file1.ini")));
        }

        // Act - not recursive - 1
        {
            let files = fs.list_files(
                &CiPath {
                    path: "".to_owned(),
                },
                Some("json"),
                false,
            );

            // Assert
            assert_eq!(files.len(), 2);
            assert!(files.contains(&root.join("file1.json")));
            assert!(files.contains(&root.join("file2.json")));
        }

        // Act - not recursive - 2
        {
            let files = fs.list_files(
                &CiPath {
                    path: "ini".to_owned(),
                },
                Some("json"),
                false,
            );

            // Assert
            assert_eq!(files.len(), 1);
            assert!(files.contains(&root.join("ini/file1.Json")));
        }

        // Act - no extension filter, recursive - returns all files in tree
        {
            let files = fs.list_files(
                &CiPath {
                    path: "".to_owned(),
                },
                None,
                true,
            );

            // Assert
            assert_eq!(files.len(), 7);
            assert!(files.contains(&root.join("file1.json")));
            assert!(files.contains(&root.join("file2.json")));
            assert!(files.contains(&root.join("file1.INI")));
            assert!(files.contains(&root.join("ini/file1.ini")));
            assert!(files.contains(&root.join("ini/file1.Json")));
            assert!(files.contains(&root.join("INNER/file1.json")));
            assert!(files.contains(&root.join("INNER/inner/file1.ini")));
        }

        // Act - no extension filter, non-recursive - returns only root-level files
        {
            let files = fs.list_files(
                &CiPath {
                    path: "".to_owned(),
                },
                None,
                false,
            );

            // Assert
            assert_eq!(files.len(), 3);
            assert!(files.contains(&root.join("file1.json")));
            assert!(files.contains(&root.join("file2.json")));
            assert!(files.contains(&root.join("file1.INI")));
        }

        // Act - no extension filter, scoped subdir, non-recursive
        {
            let files = fs.list_files(
                &CiPath {
                    path: "ini".to_owned(),
                },
                None,
                false,
            );

            // Assert
            assert_eq!(files.len(), 2);
            assert!(files.contains(&root.join("ini/file1.ini")));
            assert!(files.contains(&root.join("ini/file1.Json")));
        }

        // Act - no extension filter, scoped subdir, recursive
        {
            let files = fs.list_files(
                &CiPath {
                    path: "INNER".to_owned(),
                },
                None,
                true,
            );

            // Assert
            assert_eq!(files.len(), 2);
            assert!(files.contains(&root.join("INNER/file1.json")));
            assert!(files.contains(&root.join("INNER/inner/file1.ini")));
        }
    }
}
