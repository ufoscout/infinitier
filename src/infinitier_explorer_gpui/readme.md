# infinitier_explorer_gpui

[GPUI](https://gpui.rs/) port of `infinitier_explorer`, built on top of
the [longbridge/gpui-component](https://github.com/longbridge/gpui-component)
widget library.

Same modular layout as the egui version:

- `src/main.rs` — CLI args + bootstrap
- `src/load.rs` — opens the game folder(s) and builds the `GameData`
- `src/state.rs` — `AppState` (game data + current selection)
- `src/app.rs` — root `Render` impl that composes the panels
- `src/ui/{bottom_panel,central_panel,left_panel}.rs` — one module per panel
- `src/components/key_file_tree_view.rs` — collapsible tree of
  resources grouped by extension (mirrors the egui_ltreeview widget)
- `src/components/selected_file_info.rs` — bottom info bar
- `src/components/resource_viewer/{are, bam, bcs, …}.rs` — one
  module per resource type. The dispatcher in `mod.rs` picks the right
  viewer when the selection changes; today every viewer is a label
  stub (matching the egui `XxxViewer::show` calls that just paint
  their type name).

Run:

```sh
cargo run -p infinitier_explorer_gpui -- /path/to/baldurs/gate
```

Multiple game folders can be combined into a single case-insensitive
view by passing them comma-separated (mod-overlay order — later folders
override earlier ones).

## Build notes

`gpui` and `gpui-component` are heavy dependencies — first build can
take 10+ minutes and consume a few GB of disk. Incremental rebuilds
are fast.

On Linux, GPUI uses Wayland or X11 depending on `WAYLAND_DISPLAY` /
`DISPLAY`.
