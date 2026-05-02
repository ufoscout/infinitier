GAMES_FOLDER:="/home/ufo/Temp/Games"

# Run Infinitier Explorer Egui with local Baldur's Gate
[group('run')]
run_egui_bg:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Explorer GPUI with local Baldur's Gate
[group('run')]
run_gpui_bg:
  cargo run -p infinitier_explorer_gpui -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Explorer ICED with local Baldur's Gate
[group('run')]
run_iced_bg:
  cargo run -p infinitier_explorer_iced -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Explorer Slint with local Baldur's Gate
[group('run')]
run_slint_bg:
  cargo run -p infinitier_explorer_slint -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Explorer with local Baldur's Gate
[group('run')]
run_bg:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate/"

# Run Infinitier Explorer with local Baldur's Gate - Enhanced Edition
[group('run')]
run_bgee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate - Enhanced Edition"

# Run Infinitier Explorer with local Baldur's Gate 2
[group('run')]
run_bg2:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2/"

# Run Infinitier Explorer with local Baldur's Gate 2 - Enhanced Edition
[group('run')]
run_bg2ee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Baldur's Gate 2 - Enhanced Edition"

# Run Infinitier Explorer with local Icewind Dale
[group('run')]
run_iwd:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale"

# Run Infinitier Explorer with local Icewind Dale - Enhanced Edition
[group('run')]
run_iwdee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale - Enhanced Edition"

# Run Infinitier Explorer with local Icewind Dale 2
[group('run')]
run_iwd2:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Icewind Dale 2"

# Run Infinitier Explorer with local Planescape Torment
[group('run')]
run_pst:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Planescape Torment"

# Run Infinitier Explorer with local Planescape Torment - Enhanced Edition
[group('run')]
run_pstee:
  cargo run -p infinitier_explorer -- "{{GAMES_FOLDER}}/Planescape Torment - Enhanced Edition"