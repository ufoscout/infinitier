//! Bundle the 21 community themes from
//! [longbridge/gpui-component](https://github.com/longbridge/gpui-component/tree/main/themes)
//! into the binary at compile time, register them with
//! `ThemeRegistry` at startup, and expose a `cycle` helper the
//! top-left toggle button uses to walk through them in order.
//!
//! `gpui_component::init` already loaded the two default themes
//! (`Default Light`, `Default Dark`); `init` here adds the community
//! collections on top so the registry holds ~40 themes once it's done.

use std::rc::Rc;

use gpui::{App, SharedString, Window};
use gpui_component::{ActiveTheme, Theme, ThemeConfig, ThemeRegistry};

/// One `include_str!`-baked JSON per community theme. Each file may
/// contain several variants (e.g. Tokyo Night ships Day + Night + Storm),
/// so the registered count is higher than the file count.
const COMMUNITY_THEMES: &[(&str, &str)] = &[
    ("adventure", include_str!("../themes/adventure.json")),
    ("alduin", include_str!("../themes/alduin.json")),
    ("asciinema", include_str!("../themes/asciinema.json")),
    ("ayu", include_str!("../themes/ayu.json")),
    ("catppuccin", include_str!("../themes/catppuccin.json")),
    ("everforest", include_str!("../themes/everforest.json")),
    ("fahrenheit", include_str!("../themes/fahrenheit.json")),
    ("flexoki", include_str!("../themes/flexoki.json")),
    ("gruvbox", include_str!("../themes/gruvbox.json")),
    ("harper", include_str!("../themes/harper.json")),
    ("hybrid", include_str!("../themes/hybrid.json")),
    ("jellybeans", include_str!("../themes/jellybeans.json")),
    ("kibble", include_str!("../themes/kibble.json")),
    ("macos-classic", include_str!("../themes/macos-classic.json")),
    ("matrix", include_str!("../themes/matrix.json")),
    ("mellifluous", include_str!("../themes/mellifluous.json")),
    ("molokai", include_str!("../themes/molokai.json")),
    ("solarized", include_str!("../themes/solarized.json")),
    ("spaceduck", include_str!("../themes/spaceduck.json")),
    ("tokyonight", include_str!("../themes/tokyonight.json")),
    ("twilight", include_str!("../themes/twilight.json")),
];

/// Load every bundled community theme into the registry. Call once
/// after `gpui_component::init`.
pub fn init(cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    for (file, content) in COMMUNITY_THEMES {
        if let Err(err) = registry.load_themes_from_str(content) {
            log::warn!("Failed to load theme '{file}': {err}");
        }
    }
}

/// Advance the active theme to the next entry in the registry's
/// sorted theme list. Wraps around at the end. Returns the name of
/// the newly-active theme so the caller can refresh its UI.
pub fn cycle(window: &mut Window, cx: &mut App) -> SharedString {
    let current = cx.theme().theme_name().clone();
    let names: Vec<SharedString> = ThemeRegistry::global(cx)
        .sorted_themes()
        .iter()
        .map(|t| t.name.clone())
        .collect();
    if names.is_empty() {
        return current;
    }
    let idx = names.iter().position(|n| n == &current).unwrap_or(0);
    let next = names[(idx + 1) % names.len()].clone();
    apply_named(&next, window, cx);
    next
}

/// Apply the named theme — picked up by both `cycle` (above) and a
/// hypothetical future "theme picker" menu.
pub fn apply_named(name: &SharedString, window: &mut Window, cx: &mut App) {
    let Some(cfg): Option<Rc<ThemeConfig>> =
        ThemeRegistry::global(cx).themes().get(name).cloned()
    else {
        return;
    };
    Theme::global_mut(cx).apply_config(&cfg);
    window.refresh();
}
