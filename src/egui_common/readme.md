# infinitier_egui_common

Shared egui visual identity used by every egui-based binary in the
workspace (`infinitier_explorer`, `infinitier_keeper`, …).

The headline export is [`theme::apply`], a one-call setup that pins a
dark, Windows-11-Mica-inspired look across the application: a blue
accent, tiered grey backgrounds, larger heading / body text and tuned
spacing. Adapted from the
[`rproc`](https://github.com/Trystan-SA/rproc) project's theme module.

Use it once during app construction, typically inside the
`eframe::run_native` builder closure:

```rust
use infinitier_egui_common::theme;

let app = eframe::run_native(
    "My App",
    options,
    Box::new(|cc| {
        theme::apply(&cc.egui_ctx, &theme::DARK); // or theme::LIGHT
        Ok(Box::new(MyApp::new(cc)))
    }),
);
```

Both palettes — `theme::DARK` and `theme::LIGHT` — transcribe the
colours Slint's built-in `Palette` global emits in its Fluent style
when `color-scheme` is `Dark` or `Light` respectively. The egui
binaries pick up the same look the Slint binaries do.

For section-style widgets, [`theme::card_frame`] returns a rounded
borderless `egui::Frame` whose fill follows whichever palette is
currently applied:

```rust
theme::card_frame(ui).show(ui, |ui| {
    ui.strong("Section title");
    ui.separator();
    ui.label("…");
});
```
