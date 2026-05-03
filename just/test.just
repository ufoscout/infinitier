
# Run all tests
[group('test')]
test test_name="":
  cargo test {{test_name}}
  # cargo test {{test_name}} --all-features


# Test importing resources from real game folders
[group('test')]
test_real_imports:
  INFINITIER_GAME_DIRS="{{GAMES_FOLDER}}/Baldur's Gate/" cargo test test_import_all_resources -p infinitier_core