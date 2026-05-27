# infinitier_keeper_slint_palette

Spike — sibling of `infinitier_keeper_slint` that drops the
custom-`Theme`-global approach in favour of Slint's built-in
[`Palette`](https://docs.slint.dev/latest/docs/slint/reference/std-widgets/palette/)
global. Demonstrates two things:

1. How every panel paint can be expressed in terms of `Palette.*`
   (`background`, `alternate-background`, `accent-background`,
   `accent-foreground`, `control-background`, `control-foreground`,
   `border`, `foreground`) so the UI auto-switches with the OS
   light/dark preference — no per-color overrides needed.
2. How to **change the active scheme at runtime from Rust**:
   - On startup: `--color-scheme {auto, dark, light}` CLI flag,
     applied in [`app::run`](src/app.rs) via
     `Palette::get(&window).set_color_scheme(scheme)`.
   - Live: the **Toggle theme** button in the header fires the
     `toggle-scheme()` callback declared in `main.slint`; the Rust
     handler in [`app::toggle_color_scheme`](src/app.rs) reads the
     current value with `get_color_scheme()` and flips it. Slint's
     reactive property system repaints every binding instantly.

## Trade-offs vs. the custom-Theme sibling

| | `infinitier_keeper_slint` (custom Theme) | `infinitier_keeper_slint_palette` (built-in Palette) |
|---|---|---|
| Surface tiers | 4 (`bg`/`surface_low`/`surface`/`surface_high`) + 2 chrome | 2 (`background` / `alternate-background`) + `control-background` |
| Text tiers | 4 (`text`/`text_muted`/`text_dim`/`text_faint`) | 2 (`foreground` / `control-foreground`), or use `opacity:` for muted variants |
| Auto OS light/dark | No (have to push new values from Rust) | Yes (Palette defaults follow `ColorScheme::Unknown` = OS) |
| Custom dashboard look | Easier (curated palette) | Flatter — needs more tweaking to match a multi-tier dashboard |
| Rust binding | `window.global::<Theme>().set_*(...)` for each color | `Palette::get(&window).set_color_scheme(scheme)` — one call |

If you want a polished dashboard look with rich tiers, prefer the
custom-Theme sibling. If you want auto OS-light/dark for free and
don't mind a flatter palette, this one is the right choice.

## Key snippets

`main.slint` re-exports the built-in `Palette` so Rust can find it:

```slint
import { Palette } from "std-widgets.slint";
export { Palette }
```

Rust applies and toggles:

```rust
use slint::{ComponentHandle, Global};
use slint::language::ColorScheme;

let window = MainWindow::new()?;
// Initial scheme:
Palette::get(&window).set_color_scheme(ColorScheme::Dark);
// Live toggle (called from the on_toggle_scheme callback):
let palette = Palette::get(&window);
let next = match palette.get_color_scheme() {
    ColorScheme::Light => ColorScheme::Dark,
    _ => ColorScheme::Light,
};
palette.set_color_scheme(next);
```

## Run

```
cargo run --manifest-path src/infinitier_keeper_slint_palette/Cargo.toml \
    -- --game-path <...> --savegame <name|index>           # OS auto-scheme
cargo run --manifest-path src/infinitier_keeper_slint_palette/Cargo.toml \
    -- --game-path <...> --savegame <...> --color-scheme dark
cargo run --manifest-path src/infinitier_keeper_slint_palette/Cargo.toml \
    -- --game-path <...> --savegame <...> --color-scheme light
```

Click **Toggle theme** in the header to flip dark ↔ light without
restarting.
