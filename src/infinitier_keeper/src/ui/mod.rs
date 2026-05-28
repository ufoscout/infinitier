//! One Rust module per panel and per tab. Each module exposes a free
//! `render(this: &KeeperApp, cx: &mut Context<KeeperApp>)` that returns
//! an `IntoElement` for the surrounding `KeeperApp::render` to embed.

pub mod character;
pub mod header;
pub mod party;
pub mod tabs;
