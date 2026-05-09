//! Game detection from a game's root folder.
//!
//! Mirrors NearInfinity's `Profile.openGame` / `initGame` logic.
//!
//! The most specific markers (EE / EET / SoD)
//! come first so an enhanced edition isn't misidentified as its classic
//! ancestor. Use [`Game::engine`] to get the engine
//! (`BG / BG2 / EE / IWD / IWD2 / PST`) that the game runs on.
//!
//! ```no_run
//! use infinitier_core::fs::CaseInsensitiveFS;
//! use infinitier_core::game_detect::{detect_game, Game};
//!
//! let fs = CaseInsensitiveFS::new("/path/to/Baldur's Gate - Enhanced Edition").unwrap();
//! assert_eq!(detect_game(&fs), Some(Game::Bgee));
//! ```

use infinitier_fs::{CaseInsensitiveFS, CiPath};

pub use infinitier_common::{Engine, Game};
use log::info;

/// Detects which Infinity Engine game lives at the given root.
///
/// Returns `None` when no `chitin.key` is present, or when the game is not recognised.
pub fn detect_game(fs: &CaseInsensitiveFS) -> Option<Game> {
    let game = if !exists(fs, "chitin.key") {
        None
    } else if exists(fs, "movies/howseer.wbm") {
        Some(Game::Iwdee)
    } else if exists(fs, "data/MrtGhost.bif")
        && exists(fs, "data/shaders.bif")
        && engine_lua_mode(fs).as_deref() == Some("3")
    {
        Some(Game::Pstee)
    } else if exists(fs, "movies/pocketzz.wbm") {
        // BG2EE base install — promote to EET if its DLC markers are present.
        if exists(fs, "override/EET.flag") || exists(fs, "data/eetTU00.bif") {
            Some(Game::Eet)
        } else {
            Some(Game::Bg2ee)
        }
    } else if exists(fs, "movies/sodcin01.wbm") {
        Some(Game::BgeeSod)
    } else if exists(fs, "movies/bgenter.wbm") {
        Some(Game::Bgee)
    } else if exists(fs, "torment.exe") && !exists(fs, "movies/sigil.wbm") {
        Some(Game::Pst)
    } else if exists(fs, "idmain.exe") && !exists(fs, "movies/howseer.wbm") {
        Some(Game::Iwd)
    } else if (exists(fs, "iwd2.exe") || exists(fs, "iwd2ee.exe")) && exists(fs, "data/Credits.mve")
    {
        Some(Game::Iwd2)
    } else if exists(fs, "bg1tutu.exe") || exists(fs, "bg1mov/MovieCD1.bif") {
        Some(Game::Tutu)
    } else if exists(fs, "baldur.exe") && exists(fs, "BGConfig.exe") {
        Some(Game::Bg2)
    } else if exists(fs, "movies/graphsim.mov")
        || (exists(fs, "baldur.exe") && exists(fs, "Config.exe"))
    {
        // BG1: classic exe layout, or the Mac-build-only graphsim marker.
        Some(Game::Bg)
    } else {
        None
    };

    info!("Detected game: {game:?}");

    game
}

fn exists(fs: &CaseInsensitiveFS, path: &str) -> bool {
    fs.get_path_opt(&CiPath::new(path)).is_some()
}

/// Reads `engine_mode` out of a PSTEE/EE-style `engine.lua` if present.
///
/// The file is a tiny Lua-style key/value pair list — we don't actually
/// parse Lua, just locate a `engine_mode = <value>` entry on any line. The
/// value is returned with surrounding whitespace and trailing comments
/// stripped (so `engine_mode = 3 -- foo` returns `"3"`).
pub fn engine_lua_mode(fs: &CaseInsensitiveFS) -> Option<String> {
    let path = fs.get_path_opt(&CiPath::new("engine.lua"))?;
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
    use infinitier_test_utils::get_assets_path;

    /// Each fixture under `assets/detect_game/<dir>/` carries the minimal set
    /// of empty marker files that `detect_game` looks for — no real
    /// `chitin.key` content, hence the dedicated subtree separate from the
    /// importer fixtures.
    fn assert_detected(dir: &str, expected: Game) {
        let path = get_assets_path().join("detect_game").join(dir);
        let fs = CaseInsensitiveFS::new(&path)
            .unwrap_or_else(|e| panic!("error opening `{}`: {}", path.display(), e));
        let actual = detect_game(&fs);
        assert_eq!(
            actual,
            Some(expected),
            "detected wrong game for `{}`: expected {:?}, got {:?}",
            dir,
            expected,
            actual
        );
    }

    #[test]
    fn detect_bg() {
        assert_detected("bg", Game::Bg);
    }

    #[test]
    fn detect_bg2() {
        assert_detected("bg2", Game::Bg2);
    }

    #[test]
    fn detect_bgee() {
        assert_detected("bg_ee", Game::Bgee);
    }

    #[test]
    fn detect_bg2ee() {
        assert_detected("bg2_ee", Game::Bg2ee);
    }

    #[test]
    fn detect_bgee_sod() {
        assert_detected("bg_ee_sod", Game::BgeeSod);
    }

    #[test]
    fn detect_eet() {
        assert_detected("eet", Game::Eet);
    }

    #[test]
    fn detect_tutu() {
        assert_detected("tutu", Game::Tutu);
    }

    #[test]
    fn detect_iwd() {
        assert_detected("iwd", Game::Iwd);
    }

    #[test]
    fn detect_iwdee() {
        assert_detected("iwd_ee", Game::Iwdee);
    }

    #[test]
    fn detect_iwd2() {
        assert_detected("iwd2", Game::Iwd2);
    }

    #[test]
    fn detect_pst() {
        assert_detected("pst", Game::Pst);
    }

    #[test]
    fn detect_pstee() {
        assert_detected("pst_ee", Game::Pstee);
    }

    #[test]
    fn detect_returns_none_on_unrelated_directory() {
        // Use the workspace root as a stand-in for "obviously not a game".
        let dir = std::env::current_dir().unwrap();
        let fs = CaseInsensitiveFS::new(&dir).unwrap();
        assert_eq!(detect_game(&fs), None);
    }
}
