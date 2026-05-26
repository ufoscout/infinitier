#![doc = include_str!("../readme.md")]

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::roots::Roots;

pub mod roots;

/// A file system that is case insensitive
#[derive(Debug, Clone)]
pub struct CaseInsensitiveFS {
    /// The root directories
    roots: Vec<PathBuf>,
    /// Resource fallback folders
    fallbacks: Vec<String>,
    paths: Arc<BTreeMap<String, PathBuf>>,
}

impl CaseInsensitiveFS {
    /// Creates a new `CaseInsensitiveFS` from the given root paths.
    ///
    /// The given root paths are used as the root directories for the file system.
    /// All files and directories underneath the given root path are then
    /// traversed recursively, and their paths are stored in a map
    /// where the keys are the lowercased path strings and the values are the
    /// corresponding absolute paths.
    ///
    /// In case of conflicts, the last root is used.
    pub fn new(roots: impl Roots) -> io::Result<CaseInsensitiveFS> {
        Self::new_with_fallback(roots, vec![])
    }

    /// Returns an empty `CaseInsensitiveFS` with no roots and no
    /// indexed paths. Useful for tests / placeholders where a real
    /// game folder is not required — every path lookup on the
    /// returned FS will miss.
    pub fn empty() -> CaseInsensitiveFS {
        CaseInsensitiveFS {
            roots: Vec::new(),
            fallbacks: Vec::new(),
            paths: Arc::new(BTreeMap::new()),
        }
    }

    /// Creates a new `CaseInsensitiveFS` from the given root path.
    ///
    /// The given root paths are used as the root directories for the file system.
    /// All files and directories underneath the given root path are then
    /// traversed recursively, and their paths are stored in a map
    /// where the keys are the lowercased path strings and the values are the
    /// corresponding absolute paths.
    ///
    /// The fallbacks are used to search for files that are not found in the root directory.
    ///
    /// In case of conflicts, the last root is used.
    pub fn new_with_fallback(
        roots: impl Roots,
        fallbacks: Vec<String>,
    ) -> io::Result<CaseInsensitiveFS> {
        let roots = roots.pathbufs();
        let paths = index_roots(&roots)?;
        Ok(CaseInsensitiveFS {
            roots,
            fallbacks,
            paths: Arc::new(paths),
        })
    }

    /// Returns the roots directory of the file system.
    /// In case of conflicts, the last root is used.
    pub fn get_roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Re-walks the filesystem and refreshes the indexed path map.
    ///
    /// - `refresh(None)` rebuilds the full index from scratch by
    ///   re-walking every [`get_roots`](Self::get_roots) entry.
    /// - `refresh(Some(subpath))` only re-walks `subpath` under each
    ///   root; entries whose key is `subpath` or sits underneath it
    ///   (`subpath/…`) are dropped first, then replaced with whatever
    ///   the rescan finds. Untouched siblings keep their existing
    ///   entries — so a per-save-folder refresh after a write doesn't
    ///   force a full re-walk of the multi-thousand-file game folder.
    ///
    /// `subpath` is normalised the same way as
    /// [`get_path_opt`](Self::get_path_opt) — case-insensitive,
    /// backslash-converted, leading-slash-stripped. An empty or
    /// all-slash `subpath` is treated as `None` (full refresh).
    pub fn refresh(&mut self, subpath: Option<&str>) -> io::Result<()> {
        let normalized = subpath.map(CiPath::normalize).filter(|s| !s.is_empty());

        let fresh = match &normalized {
            None => index_roots(&self.roots)?,
            Some(sub) => self.rescan_subpath(sub)?,
        };

        self.paths = Arc::new(match normalized {
            // Full refresh — the rescan already covers every root.
            None => fresh,
            Some(sub) => {
                // Drop stale entries under the refreshed subtree, then
                // splice in the fresh ones. Anything outside the
                // subtree survives untouched.
                let prefix = format!("{sub}/");
                let mut merged: BTreeMap<String, PathBuf> = (*self.paths).clone();
                merged.retain(|key, _| !(key == sub.as_str() || key.starts_with(&prefix)));
                merged.extend(fresh);
                merged
            }
        });
        Ok(())
    }

    /// Internal helper: re-walk `normalized_sub` under every root.
    /// Roots where the subpath doesn't exist contribute no entries;
    /// that's the mechanism for picking up on-disk deletions inside
    /// the subtree.
    fn rescan_subpath(&self, normalized_sub: &str) -> io::Result<BTreeMap<String, PathBuf>> {
        let mut paths = BTreeMap::new();
        for root in &self.roots {
            let target = root.join(normalized_sub);
            if target.is_dir() {
                let canonical_root = root.canonicalize()?;
                let canonical_target = target.canonicalize()?;
                recurse(&canonical_root, &canonical_target, &mut paths)?;
            } else if target.is_file() {
                let canonical_root = root.canonicalize()?;
                let canonical_target = target.canonicalize()?;
                if let Ok(rel) = canonical_target.strip_prefix(&canonical_root)
                    && let Some(s) = rel.to_str()
                {
                    paths.insert(s.to_lowercase(), canonical_target);
                }
            }
            // Doesn't exist on this root: leave it. The caller's
            // retain() step has already dropped any stale entries —
            // an on-disk deletion inside the subtree is correctly
            // reflected as the entry going away.
        }
        Ok(paths)
    }

    /// Returns a `CiPath` for the file or directory at the given path relative to root.
    /// The path is matched case insensitively. The returned `CiPath` carries both
    /// the lowercased relative key and the canonical absolute path on disk.
    pub fn get_path_opt(&self, path: &str) -> Option<CiPath> {
        let key = CiPath::normalize(path);
        let real_path = self.paths.get(&key)?.clone();
        Some(CiPath {
            path: key,
            real_path,
        })
    }

    /// Like [`get_path_opt`](Self::get_path_opt) but returns an `io::Error` when missing.
    pub fn get_path(&self, path: &str) -> io::Result<CiPath> {
        self.get_path_opt(path).ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, format!("File not found: {path}"))
        })
    }

    /// Searches for a path in the root directory, then in each of the configured
    /// fallback folders. Returns the first match as a `CiPath`.
    pub fn search_path_opt(&self, path: &str) -> Option<CiPath> {
        if let Some(found) = self.get_path_opt(path) {
            return Some(found);
        }
        let normalized = CiPath::normalize(path);
        for dir in self.fallbacks.iter() {
            let key = CiPath::normalize(&format!("{dir}/{normalized}"));
            if let Some(real_path) = self.paths.get(&key) {
                return Some(CiPath {
                    path: key,
                    real_path: real_path.clone(),
                });
            }
        }
        None
    }

    /// Returns a list of files in a folder, optionally filtered by extension.
    /// When `extension` is `None`, all files in scope are returned.
    /// The path is matched case insensitively. When `recursive` is
    /// false, only direct children of `path` are returned; otherwise
    /// the whole subtree under `path` is walked.
    pub fn list_files(&self, path: &str, extension: Option<&str>, recursive: bool) -> Vec<CiPath> {
        let needle = CiPath::normalize(path);
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
            let push = || CiPath {
                path: key.clone(),
                real_path: value.clone(),
            };
            match extension {
                Some(ext_filter) => {
                    if let Some(ext) = value.extension()
                        && ext.eq_ignore_ascii_case(ext_filter)
                    {
                        results.push(push());
                    }
                }
                None => results.push(push()),
            }
        }
        results
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
/// A path that has been resolved through a [`CaseInsensitiveFS`].
///
/// Carries both the lowercased path relative to the FS root (used as the
/// case-insensitive lookup key) and the canonical absolute path on disk.
/// Instances are produced by [`CaseInsensitiveFS`]; external callers
/// cannot construct a `CiPath` directly.
pub struct CiPath {
    path: String,
    real_path: PathBuf,
}

impl CiPath {
    /// Normalizes a raw path string into the canonical lookup key form:
    /// trimmed, lowercased, with `\` and `:` replaced by `/` and any
    /// leading `/` stripped.
    fn normalize(path: &str) -> String {
        let mut path = path
            .trim()
            .to_lowercase()
            .replace("\\", "/")
            .replace(":", "/");
        while path.starts_with("/") {
            path = path[1..].to_string();
        }
        path
    }

    /// Returns the absolute on-disk path.
    pub fn path(&self) -> &Path {
        &self.real_path
    }

    /// Returns the lowercased path relative to the FS root.
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
        self.path.split('.').nth(1).filter(|ext| !ext.is_empty())
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

/// Walk every root in input order and merge their recursive listings
/// into a single path index. Shared by [`CaseInsensitiveFS::new_with_fallback`]
/// (initial build) and [`CaseInsensitiveFS::refresh`] (full rebuild).
fn index_roots(roots: &[PathBuf]) -> io::Result<BTreeMap<String, PathBuf>> {
    let mut paths = BTreeMap::new();
    for root in roots {
        paths.append(&mut list_real_entries_recursive(root)?);
    }
    Ok(paths)
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
        assert!(fs.get_path_opt("cargo.toml").is_some());
        assert!(fs.get_path_opt("Cargo.TOML").is_some());
        assert!(fs.get_path_opt("/cargo.TOML").is_some());
        assert!(fs.get_path_opt("/src/core/cargo.TOML").is_some());
        assert!(fs.get_path_opt("/Target").is_some());

        assert!(fs.get_path("/src/core/cargo.TOML").is_ok());
        assert!(fs.get_path("/Targets").is_err());
    }

    #[test]
    fn test_case_insensitive_fs_with_multiple_roots() {
        // Arrange — two roots, each with one unique file plus a file
        // whose case-insensitive key collides with the other root:
        // - root_a/only_a.txt          ("only_a")
        // - root_a/SHARED.txt          ("from_a")
        // - root_b/only_b.txt          ("only_b")
        // - root_b/shared.TXT          ("from_b")
        let root_a = tempfile::tempdir().unwrap();
        let root_b = tempfile::tempdir().unwrap();

        fs::write(root_a.path().join("only_a.txt"), "only_a").unwrap();
        fs::write(root_a.path().join("SHARED.txt"), "from_a").unwrap();
        fs::write(root_b.path().join("only_b.txt"), "only_b").unwrap();
        fs::write(root_b.path().join("shared.TXT"), "from_b").unwrap();

        let cifs = CaseInsensitiveFS::new(vec![root_a.path(), root_b.path()]).unwrap();

        // Both roots are recorded, in order.
        assert_eq!(
            cifs.get_roots(),
            &[
                root_a.path().canonicalize().unwrap(),
                root_b.path().canonicalize().unwrap(),
            ]
        );

        // Files unique to each root are reachable case-insensitively, and the
        // resolved paths point at the real file on disk (content check).
        let a = cifs.get_path("ONLY_A.TXT").unwrap();
        assert_eq!(
            a.path(),
            root_a.path().join("only_a.txt").canonicalize().unwrap()
        );
        assert_eq!(fs::read_to_string(a.path()).unwrap(), "only_a");

        let b = cifs.get_path("only_b.txt").unwrap();
        assert_eq!(
            b.path(),
            root_b.path().join("only_b.txt").canonicalize().unwrap()
        );
        assert_eq!(fs::read_to_string(b.path()).unwrap(), "only_b");

        // On conflict, the last root wins.
        let shared = cifs.get_path("shared.txt").unwrap();
        assert_eq!(
            shared.path(),
            root_b.path().join("shared.TXT").canonicalize().unwrap()
        );
        assert_eq!(fs::read_to_string(shared.path()).unwrap(), "from_b");
    }

    #[test]
    fn test_search_path_in_exact_path() {
        let fs =
            CaseInsensitiveFS::new(get_assets_path().join("KEY").join(BG_RESOURCES_DIR.0)).unwrap();

        let path = fs.search_path_opt("/chitin.key").unwrap();

        assert_eq!(
            path.path(),
            get_assets_path()
                .join("KEY")
                .join(BG_RESOURCES_DIR.0)
                .join("Chitin.key")
                .canonicalize()
                .unwrap()
                .as_path()
        );
    }

    #[test]
    fn test_search_path_in_fallbacks() {
        let fs = CaseInsensitiveFS::new_with_fallback(
            get_assets_path().join("KEY").join(IWD_RESOURCES_DIR.0),
            vec!["cd1".to_string(), "cd2".to_string()],
        )
        .unwrap();

        let path = fs.search_path_opt("/DATA/AR3603.cbf").unwrap();
        assert_eq!(
            path.path(),
            get_assets_path()
                .join("KEY")
                .join(IWD_RESOURCES_DIR.0)
                .join("CD2/Data/AR3603.cbf")
                .canonicalize()
                .unwrap()
                .as_path()
        );
    }

    /// Test-only helper that builds a `CiPath` from a raw string by reusing
    /// the same normalization the real lookup path uses, but skipping the
    /// filesystem round-trip so we can exercise the accessor methods in
    /// isolation.
    fn ci_path(path: &str) -> CiPath {
        CiPath {
            path: CiPath::normalize(path),
            real_path: PathBuf::new(),
        }
    }

    #[test]
    fn test_normalize() {
        // Lowercases
        assert_eq!(CiPath::normalize("Foo/Bar.EXE"), "foo/bar.exe");
        // Trims surrounding whitespace
        assert_eq!(CiPath::normalize("  data/file  "), "data/file");
        // Backslashes become forward slashes
        assert_eq!(CiPath::normalize("data\\sub\\file"), "data/sub/file");
        // Colons become forward slashes (the `\` after `C:` is also replaced,
        // and since the colon swap happens after the backslash swap, the two
        // adjacent separators in `C:\` collapse to `//` rather than a single `/`).
        assert_eq!(CiPath::normalize("C:\\Windows\\file"), "c//windows/file");
        // A standalone colon without an adjacent backslash yields a single `/`.
        assert_eq!(CiPath::normalize("C:foo"), "c/foo");
        // Leading slashes are stripped (all of them)
        assert_eq!(CiPath::normalize("/data/file"), "data/file");
        assert_eq!(CiPath::normalize("///data/file"), "data/file");
        // Empty input stays empty
        assert_eq!(CiPath::normalize(""), "");
        assert_eq!(CiPath::normalize("   "), "");
        // Bare slash collapses to empty
        assert_eq!(CiPath::normalize("/"), "");
        // Combined: trim + lowercase + backslash + leading slash
        assert_eq!(CiPath::normalize("  \\Foo\\Bar  "), "foo/bar");
    }

    #[test]
    fn test_ci_path_as_str() {
        let p = ci_path("/Data/AR3603.CBF");
        assert_eq!(p.as_str(), "data/ar3603.cbf");

        // Round-trip: as_str returns the same value used by base_name etc.
        let p = ci_path("foo/bar");
        assert_eq!(p.as_str(), "foo/bar");
    }

    #[test]
    fn test_ci_path_extension() {
        assert_eq!(ci_path("/foo/bar.exe").extension(), Some("exe"));
        assert_eq!(ci_path("file.JSON").extension(), Some("json"));
        // Empty extension after the dot returns None
        assert_eq!(ci_path("file.").extension(), None);
        // No dot at all
        assert_eq!(ci_path("foo/bar").extension(), None);
        assert_eq!(ci_path("target").extension(), None);
        // Trailing slash without dot
        assert_eq!(ci_path("foo/bar/").extension(), None);
        // Empty path
        assert_eq!(ci_path("").extension(), None);
    }

    #[test]
    fn test_ci_path_base_name_without_extension() {
        assert_eq!(ci_path("/foo/bar.exe").base_name_without_extension(), "bar");
        assert_eq!(
            ci_path("/data/AR3603.cbf").base_name_without_extension(),
            "ar3603"
        );
        // No extension
        assert_eq!(
            ci_path("/foo/target").base_name_without_extension(),
            "target"
        );
        assert_eq!(ci_path("target").base_name_without_extension(), "target");
        // Trailing slash: base_name is "", so stem is ""
        assert_eq!(ci_path("foo/bar/").base_name_without_extension(), "");
        // Empty path
        assert_eq!(ci_path("").base_name_without_extension(), "");
        assert_eq!(ci_path("/").base_name_without_extension(), "");
        // Multi-dot file: stem is everything before the first dot in the base_name
        assert_eq!(
            ci_path("/foo/archive.tar.gz").base_name_without_extension(),
            "archive"
        );
    }

    #[test]
    fn test_basename() {
        assert_eq!(ci_path("/data/AR3603.cbf").base_name(), "ar3603.cbf");
        assert_eq!(ci_path("/data/target").base_name(), "target");
        assert_eq!(ci_path("data/AR3603.cbf").base_name(), "ar3603.cbf");
        assert_eq!(ci_path("data/target").base_name(), "target");
        assert_eq!(ci_path("/AR3603.cbf").base_name(), "ar3603.cbf");
        assert_eq!(ci_path("/target").base_name(), "target");
        assert_eq!(ci_path("AR3603.cbf").base_name(), "ar3603.cbf");
        assert_eq!(ci_path("target").base_name(), "target");
        assert_eq!(ci_path("").base_name(), "");
        assert_eq!(ci_path("/").base_name(), "");
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

        fn has(files: &[CiPath], p: PathBuf) -> bool {
            files.iter().any(|f| f.path() == p)
        }

        // Act - recursive - 1
        {
            let files = fs.list_files("", Some("json"), true);

            // Assert
            assert_eq!(files.len(), 4);
            assert!(has(&files, root.join("file1.json")));
            assert!(has(&files, root.join("file2.json")));
            assert!(has(&files, root.join("ini/file1.Json")));
            assert!(has(&files, root.join("INNER/file1.json")));
        }

        // Act - recursive - 2
        {
            let files = fs.list_files("INNER", Some("json"), true);

            // Assert
            assert_eq!(files.len(), 1);
            assert!(has(&files, root.join("INNER/file1.json")));
        }

        // Act - recursive - 3
        {
            let files = fs.list_files("INNER", Some("ini"), true);

            // Assert
            assert_eq!(files.len(), 1);
            assert!(has(&files, root.join("INNER/inner/file1.ini")));
        }

        // Act - recursive - 4
        {
            let files = fs.list_files("INNER/inner", Some("ini"), true);

            // Assert
            assert_eq!(files.len(), 1);
            assert!(has(&files, root.join("INNER/inner/file1.ini")));
        }

        // Act - not recursive - 1
        {
            let files = fs.list_files("", Some("json"), false);

            // Assert
            assert_eq!(files.len(), 2);
            assert!(has(&files, root.join("file1.json")));
            assert!(has(&files, root.join("file2.json")));
        }

        // Act - not recursive - 2
        {
            let files = fs.list_files("ini", Some("json"), false);

            // Assert
            assert_eq!(files.len(), 1);
            assert!(has(&files, root.join("ini/file1.Json")));
        }

        // Act - no extension filter, recursive - returns all files in tree
        {
            let files = fs.list_files("", None, true);

            // Assert
            assert_eq!(files.len(), 7);
            assert!(has(&files, root.join("file1.json")));
            assert!(has(&files, root.join("file2.json")));
            assert!(has(&files, root.join("file1.INI")));
            assert!(has(&files, root.join("ini/file1.ini")));
            assert!(has(&files, root.join("ini/file1.Json")));
            assert!(has(&files, root.join("INNER/file1.json")));
            assert!(has(&files, root.join("INNER/inner/file1.ini")));
        }

        // Act - no extension filter, non-recursive - returns only root-level files
        {
            let files = fs.list_files("", None, false);

            // Assert
            assert_eq!(files.len(), 3);
            assert!(has(&files, root.join("file1.json")));
            assert!(has(&files, root.join("file2.json")));
            assert!(has(&files, root.join("file1.INI")));
        }

        // Act - no extension filter, scoped subdir, non-recursive
        {
            let files = fs.list_files("ini", None, false);

            // Assert
            assert_eq!(files.len(), 2);
            assert!(has(&files, root.join("ini/file1.ini")));
            assert!(has(&files, root.join("ini/file1.Json")));
        }

        // Act - no extension filter, scoped subdir, recursive
        {
            let files = fs.list_files("INNER", None, true);

            // Assert
            assert_eq!(files.len(), 2);
            assert!(has(&files, root.join("INNER/file1.json")));
            assert!(has(&files, root.join("INNER/inner/file1.ini")));
        }
    }

    #[test]
    fn refresh_full_picks_up_added_and_dropped_files() {
        let temp = tempfile::tempdir().unwrap();
        File::create(temp.path().join("seed.txt")).unwrap();
        let mut fs = CaseInsensitiveFS::new(temp.path()).unwrap();

        assert!(fs.get_path_opt("seed.txt").is_some());
        assert!(fs.get_path_opt("added.txt").is_none());

        // Mutate the filesystem outside the FS's knowledge.
        File::create(temp.path().join("added.txt")).unwrap();
        std::fs::remove_file(temp.path().join("seed.txt")).unwrap();

        // refresh(None) — full rebuild — sees both edits.
        fs.refresh(None).unwrap();
        assert!(fs.get_path_opt("added.txt").is_some());
        assert!(fs.get_path_opt("seed.txt").is_none());
    }

    #[test]
    fn refresh_subpath_only_touches_that_subtree() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir(root.join("save")).unwrap();
        File::create(root.join("save/a.txt")).unwrap();
        File::create(root.join("untouched.txt")).unwrap();
        let mut fs = CaseInsensitiveFS::new(root).unwrap();

        // Both visible after initial index.
        assert!(fs.get_path_opt("save/a.txt").is_some());
        assert!(fs.get_path_opt("untouched.txt").is_some());

        // Delete BOTH on disk, but ask for a subpath-scoped refresh.
        std::fs::remove_file(root.join("save/a.txt")).unwrap();
        std::fs::remove_file(root.join("untouched.txt")).unwrap();
        fs.refresh(Some("save")).unwrap();

        // `save/a.txt` removal is visible — the subtree was rescanned
        // and the entry is gone.
        assert!(fs.get_path_opt("save/a.txt").is_none());
        // `untouched.txt` was outside the refreshed subtree, so its
        // stale entry survives — the FS doesn't know it's gone.
        assert!(fs.get_path_opt("untouched.txt").is_some());
    }

    #[test]
    fn refresh_subpath_picks_up_new_files_under_subtree() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir(root.join("save")).unwrap();
        let mut fs = CaseInsensitiveFS::new(root).unwrap();
        assert!(fs.get_path_opt("save/new.txt").is_none());

        File::create(root.join("save/new.txt")).unwrap();
        fs.refresh(Some("Save")).unwrap(); // case-insensitive
        assert!(fs.get_path_opt("save/new.txt").is_some());
    }

    #[test]
    fn refresh_subpath_treats_empty_normalized_as_full_refresh() {
        let temp = tempfile::tempdir().unwrap();
        let mut fs = CaseInsensitiveFS::new(temp.path()).unwrap();

        File::create(temp.path().join("appeared.txt")).unwrap();
        // "/" normalises to "" — should behave like full refresh.
        fs.refresh(Some("/")).unwrap();
        assert!(fs.get_path_opt("appeared.txt").is_some());
    }

    #[test]
    fn refresh_subpath_drops_entries_when_subpath_no_longer_exists() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::create_dir(root.join("save")).unwrap();
        File::create(root.join("save/a.txt")).unwrap();
        File::create(root.join("save/b.txt")).unwrap();
        let mut fs = CaseInsensitiveFS::new(root).unwrap();

        // Whole-directory deletion: a/b should both go away after refresh.
        std::fs::remove_dir_all(root.join("save")).unwrap();
        fs.refresh(Some("save")).unwrap();
        assert!(fs.get_path_opt("save/a.txt").is_none());
        assert!(fs.get_path_opt("save/b.txt").is_none());
    }
}
