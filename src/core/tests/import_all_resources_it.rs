use infinitier_common::{Game, ResourceType};
use infinitier_core::{
    game::{DataOrigin, GameDataBuilder, GameResource},
    game_detect::detect_game, imported_resource::ImportedResource,
};
use infinitier_fs::CaseInsensitiveFS;
use infinitier_test_utils::{constants::ALL_RESOURCES_DIRS, get_assets_path};

/// Returns the list of game directories to test.
///
/// Always includes asset dirs that contain a chitin.key.
/// Otherwise, directories can be provided via the `INFINITIER_GAME_DIRS`
/// environment variable as a colon-separated list of absolute paths, e.g.:
///
///   INFINITIER_GAME_DIRS=/games/bg2:/games/bgee cargo test -p infinitier_core
fn game_dirs() -> Vec<(std::path::PathBuf, Option<Game>)> {
    let mut dirs: Vec<_> = ALL_RESOURCES_DIRS
        .iter()
        .map(|d| (get_assets_path().join(d.0), Some(d.1)))
        .filter(|p| p.0.is_dir())
        .collect();

    if let Ok(env) = std::env::var("INFINITIER_GAME_DIRS") {
        dirs.clear();
        for raw in env.split(':') {
            let p = std::path::PathBuf::from(raw.trim());
            if p.is_dir() {
                dirs.push((p, None));
            }
        }
    }

    dirs
}

#[test]
fn test_import_all_resources() {
    // start_logger();

    let dirs = game_dirs();
    assert!(!dirs.is_empty(), "No game directories found");

    let mut all_failures: Vec<String> = vec![];

    for (dir, game) in &dirs {
        let game = game.unwrap_or_else(|| {
            detect_game(&CaseInsensitiveFS::new(dir).unwrap()).expect("Cannot detect game type")
        });
        let game_data = match GameDataBuilder::new(dir, game).and_then(|b| b.build()) {
            Ok(gd) => gd,
            Err(e) => {
                all_failures.push(format!("[{}] failed to build GameData: {e}", dir.display()));
                continue;
            }
        };

        let resources = game_data.resources();
        let mut dir_failures: Vec<String> = vec![];

        for resource in resources {
            if matches!(resource.data_origin, DataOrigin::Missing) {
                continue;
            }
            let result: std::io::Result<()> = match resource.import() {
                Ok(res) => match res {
                    ImportedResource::Sound(mut sound_decoder) => {
                        sound_decoder.decode_all().map(|_| ())
                    },
                    ImportedResource::Mve(movie_source) => {
                        match movie_source.open() {
                            Ok(mut movie_decoder) => loop {
                                match movie_decoder.next_frame() {
                                    Ok(Some(_)) => continue,
                                    Ok(None) => break Ok(()),
                                    Err(e) => break Err(e.into()),
                                }
                            },
                            Err(e) => Err(e.into()),
                        }
                    },
                    _ => Ok(()),
                },
                Err(e) => Err(e)
            };

            if let Err(e) = result {
                if !expected_failures(&resource) {
                    dir_failures.push(format!("  {} — {e}", resource.filename));
                }
            }
        }

        if dir_failures.is_empty() {
            println!(
                "[{}] OK — {} resources imported",
                dir.display(),
                resources.len()
            );
        } else {
            println!(
                "[{}] {} failure(s) out of {} resources:",
                dir.display(),
                dir_failures.len(),
                resources.len()
            );
            for f in &dir_failures {
                println!("{f}");
            }
            all_failures.extend(
                dir_failures
                    .into_iter()
                    .map(|f| format!("[{}]{f}", dir.display())),
            );
        }
    }

    assert!(
        all_failures.is_empty(),
        "{} import failure(s):\n{}",
        all_failures.len(),
        all_failures.join("\n")
    );
}

/// Some resources are expected to fail to import because they are
/// corrupted.
fn expected_failures(resource: &GameResource) -> bool {
    let failures = [
        (Game::Bg, "cader09", ResourceType::Wav),
        (Game::Bg, "thunder3", ResourceType::Wav),
    ];
    
    failures.contains(&(resource.game_type, &resource.name.to_lowercase(), resource.r#type))
}
