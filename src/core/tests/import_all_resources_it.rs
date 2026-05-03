use infinitier_core::game::{DataOrigin, GameDataBuilder};
use infinitier_test_utils::{constants::ALL_RESOURCES_DIRS, get_assets_path, start_logger};

/// Returns the list of game directories to test.
///
/// Always includes asset dirs that contain a chitin.key.
/// Otherwise, directories can be provided via the `INFINITIER_GAME_DIRS`
/// environment variable as a colon-separated list of absolute paths, e.g.:
///
///   INFINITIER_GAME_DIRS=/games/bg2:/games/bgee cargo test -p infinitier_core
fn game_dirs() -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = ALL_RESOURCES_DIRS
        .iter()
        .map(|d| get_assets_path().join(d))
        .filter(|p| p.is_dir())
        .collect();

    if let Ok(env) = std::env::var("INFINITIER_GAME_DIRS") {
        dirs.clear();
        for raw in env.split(':') {
            let p = std::path::PathBuf::from(raw.trim());
            if p.is_dir() {
                dirs.push(p);
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

    for dir in &dirs {
        let game_data = match GameDataBuilder::new(dir).and_then(|b| b.build()) {
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
            if let Err(e) = resource.import() {
                dir_failures.push(format!("  {} — {e}", resource.filename));
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
