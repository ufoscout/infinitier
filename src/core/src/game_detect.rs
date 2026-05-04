//! Game detection from a game's root folder.
//!
//! Mirrors NearInfinity's `Profile.openGame` / `initGame` logic
//! (`org.infinity.resource.Profile`): given a [`CaseInsensitiveFS`] rooted at
//! a game install (the directory containing `chitin.key`), [`detect_game`]
//! probes for distinctive marker files and returns which Infinity Engine
//! game lives there.
//!
//! The probe order matches NI's: the most specific markers (EE / EET / SoD)
//! come first so an enhanced edition isn't misidentified as its classic
//! ancestor. Use [`Game::engine`] to get the broader engine bucket
//! (`BG / BG2 / EE / IWD / IWD2 / PST`) the BAF compiler / decompiler
//! contexts care about.
//!
//! ```no_run
//! use infinitier_core::fs::CaseInsensitiveFS;
//! use infinitier_core::game_detect::{detect_game, Game};
//!
//! let fs = CaseInsensitiveFS::new("/path/to/Baldur's Gate - Enhanced Edition").unwrap();
//! assert_eq!(detect_game(&fs), Some(Game::Bgee));
//! ```

use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};

// `Game` and `Engine` live in `infinitier_common` so importer crates can
// depend on them without pulling in core. Re-export here for the convenience
// of callers that already use this module.
pub use infinitier_common::{Engine, Game};

/// Detects which Infinity Engine game lives at the given root.
///
/// Returns `None` when no `chitin.key` is present, or when no recognised
/// marker file matches (e.g. an unknown / heavily-modded install). The
/// probe order mirrors NI's `Profile.initGame` so the most specific
/// release wins — for instance an EET install is recognised as `Eet`
/// rather than as `Bg2ee`.
pub fn detect_game(fs: &CaseInsensitiveFS) -> Option<Game> {
    // Every IE game has a chitin.key in the root. If it's missing, this
    // isn't a recognisable install.
    if !exists(fs, "chitin.key") {
        return None;
    }

    // The order below reproduces NI's `initGame` — most specific first.
    if exists(fs, "movies/howseer.wbm") {
        return Some(Game::Iwdee);
    }

    if exists(fs, "data/MrtGhost.bif")
        && exists(fs, "data/shaders.bif")
        && engine_lua_mode(fs).as_deref() == Some("3")
    {
        return Some(Game::Pstee);
    }

    if exists(fs, "movies/pocketzz.wbm") {
        // BG2EE base install — promote to EET if its DLC markers are present.
        if exists(fs, "override/EET.flag") || exists(fs, "data/eetTU00.bif") {
            return Some(Game::Eet);
        }
        return Some(Game::Bg2ee);
    }

    if exists(fs, "movies/sodcin01.wbm") {
        return Some(Game::BgeeSod);
    }

    if exists(fs, "movies/bgenter.wbm") {
        return Some(Game::Bgee);
    }

    if exists(fs, "torment.exe") && !exists(fs, "movies/sigil.wbm") {
        return Some(Game::Pst);
    }

    if exists(fs, "idmain.exe") && !exists(fs, "movies/howseer.wbm") {
        return Some(Game::Iwd);
    }

    if (exists(fs, "iwd2.exe") || exists(fs, "iwd2ee.exe"))
        && exists(fs, "data/Credits.mve")
    {
        return Some(Game::Iwd2);
    }

    if exists(fs, "bg1tutu.exe") || exists(fs, "bg1mov/MovieCD1.bif") {
        return Some(Game::Tutu);
    }

    if exists(fs, "baldur.exe") && exists(fs, "BGConfig.exe") {
        return Some(Game::Bg2);
    }

    // BG1: classic exe layout, or the Mac-build-only graphsim marker.
    if exists(fs, "movies/graphsim.mov")
        || (exists(fs, "baldur.exe") && exists(fs, "Config.exe"))
    {
        return Some(Game::Bg);
    }

    None
}

/// Helper: case-insensitive existence check on the FS.
fn exists(fs: &CaseInsensitiveFS, path: &str) -> bool {
    fs.get_path_opt(&CaseInsensitivePath::new(path)).is_some()
}

/// Reads `engine_mode` out of a PSTEE/EE-style `engine.lua` if present.
///
/// The file is a tiny Lua-style key/value pair list — we don't actually
/// parse Lua, just locate a `engine_mode = <value>` entry on any line. The
/// value is returned with surrounding whitespace and trailing comments
/// stripped (so `engine_mode = 3 -- foo` returns `"3"`).
pub fn engine_lua_mode(fs: &CaseInsensitiveFS) -> Option<String> {
    let path = fs.get_path_opt(&CaseInsensitivePath::new("engine.lua"))?;
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("engine_mode") else {
            continue;
        };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let mut value = rest.trim().to_string();
        // Strip trailing Lua-style line comments.
        if let Some(idx) = value.find("--") {
            value.truncate(idx);
        }
        let value = value.trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn games_root() -> PathBuf {
        std::env::var("INFINITY_GAMES_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/home/ufo/Temp/Games"))
    }

    /// Builds a [`CaseInsensitiveFS`] for `<games_root>/<name>` and runs
    /// `detect_game`. Returns `Ok(None)` when the directory isn't present
    /// so the test can skip cleanly on machines without the corpus.
    fn detect(name: &str) -> std::io::Result<Option<Game>> {
        let path = games_root().join(name);
        if !path.is_dir() {
            return Ok(None);
        }
        let fs = CaseInsensitiveFS::new(&path)?;
        Ok(detect_game(&fs))
    }

    fn assert_detected(name: &str, expected: Game) {
        match detect(name) {
            Ok(Some(actual)) => assert_eq!(
                actual, expected,
                "detected wrong game for `{}`: expected {:?}, got {:?}",
                name, expected, actual
            ),
            Ok(None) => eprintln!("skip detect_game test for `{}`: not present", name),
            Err(e) => panic!("error opening `{}`: {}", name, e),
        }
    }

    #[test]
    fn detect_bg() {
        assert_detected("Baldur's Gate", Game::Bg);
    }

    #[test]
    fn detect_bg2() {
        assert_detected("Baldur's Gate 2", Game::Bg2);
    }

    #[test]
    fn detect_bgee() {
        assert_detected("Baldur's Gate - Enhanced Edition", Game::Bgee);
    }

    #[test]
    fn detect_bg2ee() {
        assert_detected("Baldur's Gate 2 - Enhanced Edition", Game::Bg2ee);
    }

    #[test]
    fn detect_iwd() {
        assert_detected("Icewind Dale", Game::Iwd);
    }

    #[test]
    fn detect_iwdee() {
        assert_detected("Icewind Dale - Enhanced Edition", Game::Iwdee);
    }

    #[test]
    fn detect_iwd2() {
        assert_detected("Icewind Dale 2", Game::Iwd2);
    }

    #[test]
    fn detect_pst() {
        assert_detected("Planescape Torment", Game::Pst);
    }

    #[test]
    fn detect_pstee() {
        assert_detected("Planescape Torment - Enhanced Edition", Game::Pstee);
    }

    #[test]
    fn detect_returns_none_on_unrelated_directory() {
        // Use the workspace root as a stand-in for "obviously not a game".
        let dir = std::env::current_dir().unwrap();
        let fs = CaseInsensitiveFS::new(&dir).unwrap();
        assert_eq!(detect_game(&fs), None);
    }
}
