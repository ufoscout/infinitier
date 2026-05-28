
# Run all tests
[group('test')]
test test_name="":
  cargo test {{test_name}}
  # cargo test {{test_name}} --all-features


# Test importing resources from real game folders
[group('test')]
test_real_imports:
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Baldur's Gate/" cargo test test_import_all_resources --release -p infinitier_core
  INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition/" cargo test test_import_all_resources --release -p infinitier_core
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Baldur's Gate 2/" cargo test test_import_all_resources --release -p infinitier_core
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition/" cargo test test_import_all_resources --release -p infinitier_core
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Icewind Dale" cargo test test_import_all_resources --release -p infinitier_core
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition/" cargo test test_import_all_resources --release -p infinitier_core
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Icewind Dale 2" cargo test test_import_all_resources --release -p infinitier_core
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Planescape Torment" cargo test test_import_all_resources --release -p infinitier_core
  # INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition" cargo test test_import_all_resources --release -p infinitier_core
