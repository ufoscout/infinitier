# infinitier_explorer_slint

Slint port of the `infinitier_explorer` resource browser.

Same modular layout as the egui original:

- `src/main.rs` parses CLI args and boots
- `src/load.rs` opens the game folder and builds `GameData`
- `src/state.rs` holds the loaded state
- `src/app.rs` owns the `MainWindow` and wires callbacks
- `src/ui/{tree,info,viewer}.rs` populate the three on-screen panels
- `src/ui/viewers/<type>.rs` — one Rust module per resource viewer

The `.slint` markup lives under `ui/` and mirrors the same split:
`ui/main.slint` composes panels declared under `ui/panels/`, which in
turn embed shared widgets from `ui/widgets/` and per-resource viewers
from `ui/viewers/`.

Run:

```sh
cargo run -p infinitier_explorer_slint -- /path/to/baldurs/gate
```
