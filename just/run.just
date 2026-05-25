GAMES_FOLDER:="/home/ufo/Temp/Games"

# Run Infinitier Explorer with local Baldur's Gate
[group('run_explorer')]
run_bg:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Explorer with local Baldur's Gate - Enhanced Edition
[group('run_explorer')]
run_bgee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition"

# Run Infinitier Explorer with local Baldur's Gate 2
[group('run_explorer')]
run_bg2:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2/"

# Run Infinitier Explorer with local Baldur's Gate 2 - Enhanced Edition
[group('run_explorer')]
run_bg2ee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition"

# Run Infinitier Explorer with local Icewind Dale
[group('run_explorer')]
run_iwd:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale"

# Run Infinitier Explorer with local Icewind Dale - Enhanced Edition
[group('run_explorer')]
run_iwdee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition"

# Run Infinitier Explorer with local Icewind Dale 2
[group('run_explorer')]
run_iwd2:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale 2"

# Run Infinitier Explorer with local Planescape Torment
[group('run_explorer')]
run_pst:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Planescape Torment"

# Run Infinitier Explorer with local Planescape Torment - Enhanced Edition
[group('run_explorer')]
run_pstee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition"


## SAVE GAME EDITOR 

# Run Infinitier Keeper with local Baldur's Gate
[group('run_keeper')]
run_bg_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Baldur's Gate/" --save-path "./assets/SAV_GAM/bg/Save/888888890-Alone with Imoen"

# Run Infinitier Keeper with local Baldur's Gate - Enhanced Edition
[group('run_keeper')]
run_bgee_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition" --save-path "./assets/SAV_GAM/bg_ee/save/000000010-Inizio Capitolo 3"

