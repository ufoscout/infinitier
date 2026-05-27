//! One Rust module per top-level panel. Each module exposes a free
//! `render(this: &mut ExplorerApp, cx: &mut Context<ExplorerApp>)`
//! that returns an `IntoElement` for the root `ExplorerApp::render` to
//! embed — same shape as the keeper_gpui port.

pub mod bottom_panel;
pub mod central_panel;
pub mod left_panel;
