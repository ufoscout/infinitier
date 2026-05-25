GAMES_FOLDER:="/home/ufo/Temp/Games"

# Run Infinitier Explorer with local Baldur's Gate
[group('run_explorer')]
run_bg:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Explorer with local Baldur's Gate - Enhanced Edition
[group('run_explorer')]
run_bgee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition","{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Baldur's Gate - Enhanced Edition"

# Run Infinitier Explorer with local Baldur's Gate 2
[group('run_explorer')]
run_bg2:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2/"

# Run Infinitier Explorer with local Baldur's Gate 2 - Enhanced Edition
[group('run_explorer')]
run_bg2ee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition","{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Baldur's Gate II - Enhanced Edition"

# Run Infinitier Explorer with local Icewind Dale
[group('run_explorer')]
run_iwd:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale"

# Run Infinitier Explorer with local Icewind Dale - Enhanced Edition
[group('run_explorer')]
run_iwdee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition","{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Icewind Dale - Enhanced Edition"

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
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition","{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Planescape Torment - Enhanced Edition"


## SAVE GAME EDITOR 

# Run Infinitier Keeper with local Baldur's Gate
[group('run_keeper')]
run_bg_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Baldur's Gate/" --save-path "./assets/SAV_GAM/bg/Save/888888890-Alone with Imoen"

# Run Infinitier Keeper with local Baldur's Gate - Enhanced Edition
[group('run_keeper')]
run_bgee_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition","{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Baldur's Gate - Enhanced Edition" --save-path "./assets/SAV_GAM/bg_ee/save/000000010-Inizio Capitolo 3"

# Run Infinitier Keeper with local Baldur's Gate 2
[group('run_keeper')]
run_bg2_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Baldur's Gate 2/" --save-path "./assets/SAV_GAM/bg2/save/000000007-The Three of Us"

# Run Infinitier Keeper with local Baldur's Gate 2 - Enhanced Edition
[group('run_keeper')]
run_bg2ee_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition","{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Baldur's Gate - Enhanced Edition" --save-path "./assets/SAV_GAM/bg2_ee/save/000000005-Salvataggio Finale-TOB - Thu Mar 26 21-32-51 2026"

# Run Infinitier Keeper with local Icewind Dale
[group('run_keeper')]
run_iwd_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Icewind Dale" --save-path "./assets/SAV_GAM/iwd/mpsave/000000001-Quick-Save"

# Run Infinitier Keeper with local Icewind Dale - Enhanced Edition
[group('run_keeper')]
run_iwdee_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition","{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Icewind Dale - Enhanced Edition" --save-path "./assets/SAV_GAM/iwdee/save/000000001-Quick-Save-2"

# Run Infinitier ExplKeeperorer with local Icewind Dale 2
[group('run_keeper')]
run_iwd2_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Icewind Dale 2" --save-path "./assets/SAV_GAM/iwd2/mpsave/000000002-Quick-Save"

# Run Infinitier Keeper with local Planescape Torment
[group('run_keeper')]
run_pst_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Planescape Torment" --save-path "./assets/SAV_GAM/pst/save/000000001-Quick-Save"

# Run Infinitier Keeper with local Planescape Torment - Enhanced Edition
[group('run_keeper')]
run_pstee_keeper:
  cargo run -p infinitier_keeper -- --game-path "{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition","{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Planescape Torment - Enhanced Edition" --save-path "./assets/SAV_GAM/pst_ee/save/000000001-Quick-Save"