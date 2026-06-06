//! Embedded GemRB "unhardcoded" resources.
//!
//! GemRB ships a tree of resources (under `unhardcoded/<game>` and
//! `unhardcoded/shared`) that recreate behaviour the original Infinity Engine
//! kept hardcoded in its executable — class abilities (bardsong, turn undead,
//! sneak attack…), bonus/progression tables, projectiles, visual effects, and
//! so on. GemRB loads them at *lower* priority than the real game data, so the
//! game's own files always win where they exist; the embedded set only fills
//! gaps. See `unhardcoded/README.md` for the upstream description.
//!
//! This module embeds that tree into the binary at compile time (the on-disk
//! copy is refreshed with `just update-unhardcoded`) and exposes it keyed by
//! [`Game`], so [`GameDataBuilder`](crate::game::GameDataBuilder) can layer the
//! resources in as fallbacks.
//!
//! Each [`Game`] resolves to *at most one* game-specific folder plus the
//! always-applicable `shared` folder. Game-specific entries take precedence
//! over `shared` ones with the same name and type, mirroring GemRB's ordering.
//!
//! ```no_run
//! use infinitier_common::Game;
//! use infinitier_core::unhardcoded::unhardcoded_resources;
//!
//! for res in unhardcoded_resources(Game::Bg) {
//!     let ds = res.data_source(); // ready to import / register
//!     let _ = (res.name, res.r#type, ds);
//! }
//! ```

use std::collections::HashSet;

use include_dir::{Dir, include_dir};
use infinitier_common::{Game, ResourceType};
use infinitier_datasource::DataSource;

/// The whole gemrb `unhardcoded` tree, embedded at compile time.
static UNHARDCODED: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/unhardcoded");

/// A single embedded unhardcoded resource.
///
/// `bytes` points straight at the data baked into the binary, so building a
/// [`DataSource`] (via [`data_source`](Self::data_source)) or reading it is
/// allocation-free until the bytes are actually consumed.
#[derive(Debug, Clone, Copy)]
pub struct UnhardcodedResource {
    /// Resource name, lowercased and without extension (matching the
    /// convention used by [`GameResource`](crate::game::GameResource)).
    pub name: &'static str,
    /// Resource type, derived from the file extension.
    pub r#type: ResourceType,
    /// The subfolder the resource came from (`"shared"`, `"bg1"`, …).
    pub folder: &'static str,
    /// The embedded file contents.
    pub bytes: &'static [u8],
}

impl UnhardcodedResource {
    /// A [`DataSource`] over the embedded bytes, ready to import or register.
    pub fn data_source(&self) -> DataSource {
        DataSource::new(self.bytes)
    }
}

/// The game-specific `unhardcoded` folder name for `game`.
fn game_folder(game: Game) -> &'static str {
    match game {
        Game::Bg => "bg1",
        // Tutu runs BG1 content on the BG2 engine; it shares BG2's set.
        Game::Bg2 | Game::Tutu => "bg2",
        Game::Bgee | Game::BgeeSod => "bgee",
        Game::Bg2ee | Game::Eet => "bg2ee",
        Game::Iwd {
            heart_of_winter: true,
            ..
        } => "how",
        Game::Iwd {
            heart_of_winter: false,
            ..
        } => "iwd",
        Game::Iwd2 => "iwd2",
        Game::Pst => "pst",
        Game::Iwdee => "iwdee",
        Game::Pstee => "pstee",
    }
}

/// The embedded unhardcoded resources that apply to `game`.
///
/// Yields the game-specific set (if any) followed by the `shared` set, with
/// `shared` entries that collide with a game-specific resource of the same
/// name and type dropped — so the returned slice has no `(name, type)`
/// duplicates and game-specific resources win, matching GemRB's priority.
///
/// Files whose extension doesn't map to a known [`ResourceType`] (e.g. the
/// folder's `README.md`) are skipped.
pub fn unhardcoded_resources(game: Game) -> Vec<UnhardcodedResource> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    collect_dir(game_folder(game), &mut out, &mut seen);
    collect_dir("shared", &mut out, &mut seen);

    out
}

/// Looks up a single embedded resource for `game` by name and type, searching
/// the game-specific folder first and then `shared`. The name is matched
/// case-insensitively.
pub fn unhardcoded_resource(
    game: Game,
    name: &str,
    r#type: ResourceType,
) -> Option<UnhardcodedResource> {
    let folders = [game_folder(game), "shared"];
    for folder in folders {
        if let Some(res) = find_in_dir(folder, name, r#type) {
            return Some(res);
        }
    }
    None
}

/// Appends every recognised top-level resource of `folder` to `out`, skipping
/// any whose `(name, type)` is already in `seen`. Nested directories (e.g.
/// `pst/source`) are intentionally ignored — only loadable resources at the
/// top level of a folder are treated as unhardcoded data.
fn collect_dir(
    folder: &'static str,
    out: &mut Vec<UnhardcodedResource>,
    seen: &mut HashSet<(&'static str, ResourceType)>,
) {
    let Some(dir) = UNHARDCODED.get_dir(folder) else {
        return;
    };
    for file in dir.files() {
        let Some((name, r#type)) = parse_entry(file) else {
            continue;
        };
        if seen.insert((name, r#type)) {
            out.push(UnhardcodedResource {
                name,
                r#type,
                folder,
                bytes: file.contents(),
            });
        }
    }
}

/// Finds a single resource by name and type within `folder`.
fn find_in_dir(
    folder: &'static str,
    name: &str,
    r#type: ResourceType,
) -> Option<UnhardcodedResource> {
    let dir = UNHARDCODED.get_dir(folder)?;
    dir.files().find_map(|file| {
        let (entry_name, entry_type) = parse_entry(file)?;
        (entry_type == r#type && entry_name.eq_ignore_ascii_case(name)).then_some(
            UnhardcodedResource {
                name: entry_name,
                r#type: entry_type,
                folder,
                bytes: file.contents(),
            },
        )
    })
}

/// Extracts the extension-less name and the [`ResourceType`] from an embedded
/// file, or `None` if the extension is unknown. Embedded gemrb names are
/// already lowercase.
fn parse_entry(file: &'static include_dir::File<'static>) -> Option<(&'static str, ResourceType)> {
    let path = file.path();
    let stem = path.file_stem()?.to_str()?;
    let ext = path.extension()?.to_str()?;
    let r#type = ResourceType::from_extension(ext)?;
    Some((stem, r#type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_shared_and_game_specific() {
        let bg = unhardcoded_resources(Game::Bg);
        assert!(!bg.is_empty(), "BG should have embedded resources");

        // `bardsong.spl` is a shared GemRB recreation of a hardcoded ability.
        assert!(
            bg.iter()
                .any(|r| r.name == "bardsong" && r.r#type == ResourceType::Spl),
            "expected the shared bardsong.spl to be present for BG"
        );
    }

    #[test]
    fn no_duplicate_name_and_type() {
        let res = unhardcoded_resources(Game::Bg2);
        let mut seen = HashSet::new();
        for r in &res {
            assert!(
                seen.insert((r.name, r.r#type)),
                "duplicate (name, type) in result: {} {:?}",
                r.name,
                r.r#type
            );
        }
    }

    #[test]
    fn heart_of_winter_uses_how_folder() {
        let iwd = unhardcoded_resources(Game::Iwd {
            heart_of_winter: false,
            totl: false,
        });
        let how = unhardcoded_resources(Game::Iwd {
            heart_of_winter: true,
            totl: false,
        });
        // IWD and HoW resolve to different embedded folders of different sizes.
        assert_ne!(
            iwd.len(),
            how.len(),
            "IWD and HoW should resolve to different embedded folders"
        );
    }

    #[test]
    fn single_lookup_matches_case_insensitively() {
        let res = unhardcoded_resource(Game::Bg, "BARDSONG", ResourceType::Spl);
        assert!(
            res.is_some(),
            "case-insensitive lookup should find bardsong"
        );
    }
}
