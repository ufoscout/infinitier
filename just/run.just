GAMES_FOLDER:="/home/ufo/Temp/Games"

# RunInfinitier Explorerwith local Baldur's Gate
[group('run')]
run_bg:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# RunInfinitier Explorerwith local Baldur's Gate - Enhanced Edition
[group('run')]
run_bgee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition"

# RunInfinitier Explorerwith local Baldur's Gate 2
[group('run')]
run_bg2:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2/"

# RunInfinitier Explorerwith local Baldur's Gate 2 - Enhanced Edition
[group('run')]
run_bg2ee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition"

# RunInfinitier Explorerwith local Icewind Dale
[group('run')]
run_iwd:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale"

# RunInfinitier Explorerwith local Icewind Dale - Enhanced Edition
[group('run')]
run_iwdee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition"

# RunInfinitier Explorerwith local Icewind Dale 2
[group('run')]
run_iwd2:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale 2"

# RunInfinitier Explorerwith local Planescape Torment
[group('run')]
run_pst:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Planescape Torment"

# RunInfinitier Explorerwith local Planescape Torment - Enhanced Edition
[group('run')]
run_pstee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition"