use infinitier_common::{Game, ResourceType};
use infinitier_core::{
    game::{DataOrigin, GameDataBuilder, GameResource},
    game_detect::detect_game,
    imported_resource::ImportedResource,
};
use infinitier_fs::CaseInsensitiveFS;

/// Returns the list of game directories to test, read from the
/// `INFINITIER_GAME_DIRS` environment variable as a colon-separated
/// list of absolute paths:
///
///   INFINITIER_GAME_DIRS=/games/bg2:/games/bgee cargo test -p infinitier_core
///
/// Returns `None` when the env var is not set, so the caller can emit
/// a clear skip message instead of silently passing or pulling in
/// partial repo fixtures (which would produce noisy false negatives
/// because shipped BIFs are deliberately incomplete).
fn game_dirs() -> Option<Vec<std::path::PathBuf>> {
    let raw = std::env::var("INFINITIER_GAME_DIRS").ok()?;
    let dirs: Vec<std::path::PathBuf> = raw
        .split(':')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();
    Some(dirs)
}

#[test]
fn test_import_all_resources() {
    // start_logger();

    let Some(dirs) = game_dirs() else {
        eprintln!(
            "skipping test_import_all_resources: set INFINITIER_GAME_DIRS=/path/to/game1:/path/to/game2 to run it against full game installs"
        );
        return;
    };
    assert!(
        !dirs.is_empty(),
        "INFINITIER_GAME_DIRS is set but no listed directory exists"
    );

    let mut all_failures: Vec<String> = vec![];

    for dir in &dirs {
        // The env var only carries paths, so the engine is always
        // detected from the directory contents.
        let game =
            detect_game(&CaseInsensitiveFS::new(dir).unwrap()).expect("Cannot detect game type");
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
            let result: std::io::Result<()> = match resource.import(&game_data) {
                Ok(res) => match res.into_owned() {
                    ImportedResource::Sound(mut sound_decoder) => {
                        sound_decoder.decode_all().map(|_| ())
                    }
                    ImportedResource::Mve(movie_source) => match movie_source.open() {
                        Ok(mut movie_decoder) => loop {
                            match movie_decoder.next_frame() {
                                Ok(Some(_)) => continue,
                                Ok(None) => break Ok(()),
                                Err(e) => break Err(e.into()),
                            }
                        },
                        Err(e) => Err(e.into()),
                    },
                    _ => Ok(()),
                },
                Err(e) => Err(e),
            };

            if let Err(e) = result
                && !expected_failures(resource)
            {
                dir_failures.push(format!(
                    "  {} — {e}",
                    resource.resource_name_with_extension()
                ));
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

    failures.contains(&(
        resource.game_type,
        &resource.name.to_lowercase(),
        resource.r#type,
    ))
}
