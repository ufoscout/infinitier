use std::io;

use infinitier_bcs_resource::{Bcs, baf::BafContext, signatures::Signatures};
use infinitier_common::ResourceType;
use infinitier_ids_importer::Ids;

use crate::game::GameData;
use crate::imported_resource::ImportedResource;

/// A preloaded BCS script: the parsed bytecode plus its decompiled BAF
/// source.
#[derive(Debug)]
pub struct ImportedBcs {
    /// The original parsed BCS bytecode.
    pub bcs: Bcs,
    /// BAF source decompiled from [`Self::bcs`]. Quality of symbol
    /// resolution depends on which IDS resources are available in the
    /// supplying `GameData` — unresolved object identifiers fall back to
    /// `UnknownObject<n>` and raw integers, matching NearInfinity's
    /// behaviour with an empty IDS cache.
    pub baf: String,
}

impl ImportedBcs {
    /// Build an [`ImportedBcs`] from a parsed [`Bcs`]. Looks up
    /// TRIGGER.IDS / ACTION.IDS in `game_data` to compile the function
    /// signature tables and returns an error only when they are missing.
    /// The decompiler tolerates a missing OBJECT.IDS and the like by
    /// falling back to numeric output.
    pub fn load(bcs: Bcs, game_data: &GameData) -> io::Result<Self> {
        let game = game_data.game();
        let triggers_ids = load_ids(game_data, "trigger")?;
        let actions_ids = load_ids(game_data, "action")?;
        let triggers = Signatures::from_ids(&triggers_ids);
        let actions = Signatures::from_ids(&actions_ids);

        // 4-space indent matches the NearInfinity-style reference BAFs
        // committed in `assets/BCS/source/` and reads nicer in the
        // explorer viewer than the default `\t`.
        let mut ctx = BafContext::new(triggers, actions, game).with_indent("    ");

        // Layer in every other IDS file we can find.
        for resource in game_data.get_all_by_type(ResourceType::Ids) {
            if resource.name == "trigger" || resource.name == "action" {
                continue;
            }
            if let Ok(ImportedResource::Ids(ids)) = resource.import(game_data) {
                ctx = ctx.with_ids(&resource.name, ids);
            }
        }

        let baf = bcs.to_baf(&ctx);
        Ok(Self { bcs, baf })
    }
}

fn load_ids(game_data: &GameData, name: &str) -> io::Result<Ids> {
    let resource = game_data
        .get_by_name_and_type(name, ResourceType::Ids)
        .ok_or_else(|| {
            io::Error::other(format!(
                "IDS resource '{name}' not found in GameData (required to decompile BCS to BAF)"
            ))
        })?;
    match resource.import(game_data)? {
        ImportedResource::Ids(ids) => Ok(ids),
        _ => Err(io::Error::other(format!(
            "resource '{name}' is not an IDS resource"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use infinitier_bcs_resource::BcsImporter;
    use infinitier_common::Game;
    use infinitier_datasource::{DataSource, Importer};
    use infinitier_test_utils::get_assets_path;

    use crate::game::{DataOrigin, GameResource};

    use super::*;

    /// Build a [`GameData`] populated with every `*.IDS` file found in
    /// `ids_dir`. Names are stored lowercased to match
    /// [`GameData::get_by_name_and_type`]'s indexing convention. Used to
    /// drive `ImportedBcs::load` against a directory-based corpus
    /// without having to enumerate every IDS file by hand.
    fn make_game_data_from_dir(ids_dir: &Path) -> GameData {
        let mut resources = vec![];
        for entry in std::fs::read_dir(ids_dir).expect("read ids dir") {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            if ext.as_deref() != Some("ids") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let file_size = path.metadata().ok().map(|m| m.len());
            resources.push(GameResource {
                game_type: Game::Bg2,
                name: stem.to_ascii_lowercase(),
                r#type: ResourceType::Ids,
                file_size,
                datasource: Some(DataSource::new(path.as_path())),
                data_origin: DataOrigin::Missing,
            });
        }
        GameData::new(resources, Game::Bg2)
    }

    fn import_bcs(path: &Path) -> Bcs {
        let ds = DataSource::new(path);
        BcsImporter { name: "test_bcs" }.import(&ds).unwrap()
    }

    #[test]
    fn test_load_bcs_decompiles_to_baf() {
        // The corpus folder ships TRIGGER.IDS, ACTION.IDS and the
        // standard set of resolution IDS files (OBJECT, EA, …) so the
        // test mirrors the symbol resolution available in a real game.
        let corpus_dir = get_assets_path().join("BCS").join("source");
        let game_data = make_game_data_from_dir(&corpus_dir);

        let bcs_path = corpus_dir.join("ABAZIGAL.BCS");
        let bcs = import_bcs(&bcs_path);
        // Re-parse independently so we can compare against the field on
        // `ImportedBcs` after `load` takes ownership.
        let original = import_bcs(&bcs_path);

        let imported = ImportedBcs::load(bcs, &game_data).unwrap();

        assert_eq!(imported.bcs, original, "BCS bytecode must be preserved");

        // We don't byte-compare against the committed `ABAZIGAL.BAF` —
        // those references were produced by NearInfinity with
        // string-table / RES-name annotations (`// No such index`,
        // `// Trap Sprung`, …) that our decompiler does not emit. Verify
        // the structural markers and that symbol resolution is wired
        // through.
        assert!(
            !imported.baf.is_empty(),
            "BAF decompilation must produce non-empty output"
        );
        assert!(
            imported.baf.contains("IF\n"),
            "BAF output does not look like BAF source:\n{}",
            imported.baf
        );
        assert!(
            imported.baf.contains("END"),
            "BAF output is missing closing END:\n{}",
            imported.baf
        );
        // Symbol resolution should turn target-specifier integers into
        // the EA.IDS names (e.g. `PC`, `ENEMY`). If the IDS layering
        // broke, the output would contain raw integers and `UnknownObject<n>`
        // instead.
        assert!(
            imported.baf.contains("Myself"),
            "BAF output is missing the `Myself` object — OBJECT.IDS lookup is broken:\n{}",
            imported.baf
        );
    }

    #[test]
    fn test_load_bcs_missing_trigger_ids_errors() {
        // Empty GameData → neither TRIGGER nor ACTION are findable; the
        // error must surface the missing resource so callers can diagnose.
        let game_data = GameData::new(vec![], Game::Bg2);
        let bcs_path = get_assets_path().join("BCS").join("foozle.bcs");
        let bcs = import_bcs(&bcs_path);

        let err = ImportedBcs::load(bcs, &game_data).unwrap_err();
        assert!(
            err.to_string().contains("trigger"),
            "expected error about missing trigger IDS, got {err}"
        );
    }
}
