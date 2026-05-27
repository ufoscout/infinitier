# infinitier_keeper_gpui

[GPUI](https://gpui.rs/) port of `infinitier_keeper_slint`, built on
top of the [longbridge/gpui-component](https://github.com/longbridge/gpui-component)
widget library.

Same modular layout as the Slint version:

- `src/main.rs` — CLI args + bootstrap
- `src/load.rs` — opens the game folder and resolves the save
- `src/state.rs` — `KeeperApp` root-view state
- `src/app.rs` — root `Render` impl that composes the panels
- `src/ui/{header,party,character}.rs` — one module per panel
- `src/ui/tabs/{abilities, …}.rs` — one module per character tab
  (`abilities.rs` is fully ported; the other 14 tabs are stubs that
   say "X — not implemented yet.", matching the Slint spike)

Run:

```sh
cargo run -p infinitier_keeper_gpui -- \
    --game-path /path/to/baldurs/gate \
    --savegame 0
```

## Standalone workspace

This crate has its own `[workspace]` block — `gpui-component` pulls in
~60 git-pinned tree-sitter and shader crates that we don't want
bleeding into the main workspace's lockfile. It's excluded from the
parent workspace via `exclude` in the top-level `Cargo.toml`.

## Build notes

`gpui` and `gpui-component` are heavy dependencies — first build can
take 10+ minutes and consume a few GB of disk. Incremental rebuilds
are fast.

On Linux, GPUI uses Wayland or X11 depending on `WAYLAND_DISPLAY` /
`DISPLAY`.
