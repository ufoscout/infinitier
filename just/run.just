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
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Keeper with local Baldur's Gate - Enhanced Edition
[group('run_keeper')]
run_bgee_keeper:
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition","{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Baldur's Gate - Enhanced Edition"

# Run Infinitier Keeper with local Baldur's Gate 2
[group('run_keeper')]
run_bg2_keeper:
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Baldur's Gate 2/"

# Run Infinitier Keeper with local Baldur's Gate 2 - Enhanced Edition
[group('run_keeper')]
run_bg2ee_keeper:
  cargo run -p infinitier_keeper -- --savegame "000000004-Salvataggio Rapido-4-TOB" --game-path "{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition","{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Baldur's Gate II - Enhanced Edition"

# Run Infinitier Keeper with local Icewind Dale
[group('run_keeper')]
run_iwd_keeper:
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Icewind Dale"

# Run Infinitier Keeper with local Icewind Dale - Enhanced Edition
[group('run_keeper')]
run_iwdee_keeper:
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition","{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Icewind Dale - Enhanced Edition"

# Run Infinitier ExplKeeperorer with local Icewind Dale 2
[group('run_keeper')]
run_iwd2_keeper:
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Icewind Dale 2"

# Run Infinitier Keeper with local Planescape Torment
[group('run_keeper')]
run_pst_keeper:
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Planescape Torment"

# Run Infinitier Keeper with local Planescape Torment - Enhanced Edition
[group('run_keeper')]
run_pstee_keeper:
  cargo run -p infinitier_keeper -- --savegame 0 --game-path "{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition","{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition/wineprefix/drive_c/users/ufo/Documents/Planescape Torment - Enhanced Edition"