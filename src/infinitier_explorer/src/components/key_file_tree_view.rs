//! Resource tree grouped by file extension. The egui version uses
//! `egui_ltreeview`; here we paint our own collapsible list on top of
//! gpui's [`uniform_list`], which lazy-renders only the visible
//! subset. Without virtualization the panel produces ~17 000 DOM
//! nodes for the BAM group on a BG2EE install, which makes scrolling
//! crawl.
//!
//! Layout strategy:
//! - `KeyFileTreeView::groups` is the immutable per-extension index
//!   built once from the loaded `GameData` (sorted by extension, then
//!   leaf label — same order the egui version gets out of `BTreeMap`).
//! - On every render we flatten the groups into a `Vec<TreeRow>` based
//!   on the current `expanded_groups` set. The Vec is cheap (one
//!   small enum per row, no allocations per leaf).
//! - `uniform_list` then walks only the visible window of that Vec.
//!
//! Expansion state and the keyboard cursor (`focused_row`) live on
//! `KeyFileTreeView` itself — no other component reads them. Only
//! the chosen resource (which the central viewer + bottom info bar
//! both consume) lives on the shared `AppState`.
//!
//! Keyboard model:
//! - Wrapper div owns a `FocusHandle` + the `KEY_FILE_TREE_CONTEXT`
//!   key context. Once focused, arrow keys dispatch the gpui-component
//!   `SelectUp/Down/Left/Right` actions (bindings live in `main.rs`).
//! - Up/Down walk the flat row Vec; landing on a leaf mirrors its
//!   resource into `selected_resource` so the viewer follows the
//!   cursor.
//! - Left collapses an open header / jumps from a leaf to its header.
//! - Right expands a closed header.

use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use gpui::{
    App, Context, FocusHandle, FontWeight, InteractiveElement, IntoElement, KeyBinding,
    ListSizingBehavior, ParentElement, ScrollStrategy, StatefulInteractiveElement, Styled,
    UniformListScrollHandle, actions, div, px, uniform_list,
};
use gpui_component::{ActiveTheme, h_flex, scroll::ScrollableElement};
use infinitier_core::game::{DataOrigin, GameData};

use crate::app::ExplorerApp;

/// Key context the tree wrapper publishes. Same string the key
/// bindings in [`init`] target, so the arrow-key actions don't
/// fight with any other tree we might add later.
const KEY_FILE_TREE_CONTEXT: &str = "KeyFileTree";

// Tree-local arrow-key actions. `gpui-component` keeps its own
// `SelectUp`/etc. behind a `pub(crate)` module, so we declare our own
// unit-struct actions here and bind them in [`init`].
actions!(
    key_file_tree,
    [SelectUp, SelectDown, SelectLeft, SelectRight]
);

/// Bind the four arrow keys to the tree's own
/// `SelectUp/Down/Left/Right` actions in [`KEY_FILE_TREE_CONTEXT`].
/// Mirrors the `init()` convention `gpui-component`'s own widgets
/// use (see `tree.rs` in that crate). Call once at startup, after
/// `gpui_component::init(cx)`.
pub fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectUp, Some(KEY_FILE_TREE_CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(KEY_FILE_TREE_CONTEXT)),
        KeyBinding::new("left", SelectLeft, Some(KEY_FILE_TREE_CONTEXT)),
        KeyBinding::new("right", SelectRight, Some(KEY_FILE_TREE_CONTEXT)),
    ]);
}

pub struct KeyFileTreeView {
    groups: Vec<TreeGroup>,
    /// Extension groups currently expanded by the user. Tree-local;
    /// only this module's render + action handlers read or write it.
    expanded_groups: HashSet<&'static str>,
    /// Where the keyboard cursor currently sits. Distinct from
    /// `AppState::selected_resource` because the cursor can rest on
    /// a header (group label) where there's no resource to select.
    /// When the cursor lands on a leaf via the arrow keys we mirror
    /// the leaf into `selected_resource` so the central viewer
    /// follows along.
    focused_row: Option<FocusedRow>,
    /// Drives both the `uniform_list`'s scroll position and the
    /// `gpui-component` scrollbar painted next to it. Persistent so
    /// scroll position survives re-renders.
    scroll_handle: UniformListScrollHandle,
    /// Owns the focus token the wrapper div tracks. Once the wrapper
    /// holds focus, arrow keys dispatch into our `on_action_*`
    /// handlers.
    focus_handle: FocusHandle,
}

struct TreeGroup {
    ext: &'static str,
    entries: Vec<TreeLeaf>,
}

struct TreeLeaf {
    /// Pre-formatted leaf label including the "(O)" override marker.
    label: String,
    /// Index into `GameData::resources` — what we write into
    /// `AppState::selected_resource` when the user clicks the leaf.
    idx: usize,
}

/// A single flattened row in the visible tree. Headers and leaves
/// share the same height so `uniform_list` is happy to lay them out.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TreeRow {
    Header { group_ix: usize },
    Leaf { group_ix: usize, leaf_ix: usize },
}

/// Stable reference to a row regardless of expansion changes — kept
/// in `KeyFileTreeView::focused_row` so the keyboard cursor survives
/// a collapse / expand. `TreeRow` has the same shape today but is an
/// internal layout type; this one is the persisted form.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FocusedRow {
    Header { group_ix: usize },
    Leaf { group_ix: usize, leaf_ix: usize },
}

impl FocusedRow {
    fn from_row(row: TreeRow) -> Self {
        match row {
            TreeRow::Header { group_ix } => Self::Header { group_ix },
            TreeRow::Leaf { group_ix, leaf_ix } => Self::Leaf { group_ix, leaf_ix },
        }
    }
}

impl KeyFileTreeView {
    pub fn new(game_data: &GameData, cx: &mut Context<ExplorerApp>) -> Self {
        // BTreeMap → Vec conversion. We rely on BTreeMap's sorted
        // iteration to keep extensions alphabetical and labels in
        // case-sensitive order, matching the egui port.
        let mut tmp: BTreeMap<&'static str, BTreeMap<String, usize>> = BTreeMap::new();
        for (i, entry) in game_data.resources().iter().enumerate() {
            let ext = entry.r#type.get_extension().unwrap_or("unknown");
            let leaf_label = if matches!(entry.data_origin, DataOrigin::Dir { .. }) {
                format!("{} (O)", entry.resource_name_with_extension())
            } else {
                entry.resource_name_with_extension()
            };
            tmp.entry(ext).or_default().insert(leaf_label, i);
        }
        let groups = tmp
            .into_iter()
            .map(|(ext, entries)| TreeGroup {
                ext,
                entries: entries
                    .into_iter()
                    .map(|(label, idx)| TreeLeaf { label, idx })
                    .collect(),
            })
            .collect();
        Self {
            groups,
            expanded_groups: HashSet::new(),
            focused_row: None,
            scroll_handle: UniformListScrollHandle::default(),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Flatten the tree into the row list for the current expansion
    /// state. Cheap (~17k usize-sized entries on BG2EE) and rebuilt
    /// every frame so we don't need to invalidate caches.
    fn build_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::with_capacity(self.groups.len() + 64);
        for (group_ix, group) in self.groups.iter().enumerate() {
            rows.push(TreeRow::Header { group_ix });
            if self.expanded_groups.contains(group.ext) {
                for leaf_ix in 0..group.entries.len() {
                    rows.push(TreeRow::Leaf { group_ix, leaf_ix });
                }
            }
        }
        rows
    }

    /// Row index of a focused entry in the current rows Vec. Returns
    /// `None` if the focused row no longer exists (e.g. its group was
    /// collapsed by some other code path); callers typically clamp
    /// back onto the header in that case.
    fn focused_row_index(&self, focused: FocusedRow) -> Option<usize> {
        // Headers come at predictable offsets — walk groups in order
        // and accumulate (1 header + expanded? leaf_count) per group.
        let mut acc = 0usize;
        for (gi, group) in self.groups.iter().enumerate() {
            let is_open = self.expanded_groups.contains(group.ext);
            match focused {
                FocusedRow::Header { group_ix } if group_ix == gi => return Some(acc),
                FocusedRow::Leaf { group_ix, leaf_ix } if group_ix == gi => {
                    if !is_open {
                        return None;
                    }
                    if leaf_ix >= group.entries.len() {
                        return None;
                    }
                    return Some(acc + 1 + leaf_ix);
                }
                _ => {}
            }
            acc += 1;
            if is_open {
                acc += group.entries.len();
            }
        }
        None
    }
}

// ── Action handlers ─────────────────────────────────────────────────
//
// All four are free functions taking `&mut ExplorerApp` because the
// click + keyboard cursor landing on a leaf also has to mirror the
// resource into `AppState::selected_resource`. Wiring is done in
// `render` via `cx.listener`.

/// Move the cursor up one row (no wrap). Auto-selects when the new
/// row is a leaf.
fn on_select_up(this: &mut ExplorerApp) {
    let rows = this.tree_view.build_rows();
    if rows.is_empty() {
        return;
    }
    let current_ix = current_row_index(this, &rows);
    let new_ix = match current_ix {
        Some(ix) if ix > 0 => ix - 1,
        // Nothing focused yet, or already at top — land on the first
        // row so a fresh `Up` press has a sensible effect.
        _ => 0,
    };
    apply_cursor(this, &rows, new_ix);
}

/// Move the cursor down one row (no wrap).
fn on_select_down(this: &mut ExplorerApp) {
    let rows = this.tree_view.build_rows();
    if rows.is_empty() {
        return;
    }
    let current_ix = current_row_index(this, &rows);
    let new_ix = match current_ix {
        Some(ix) if ix + 1 < rows.len() => ix + 1,
        // No cursor yet, or already at bottom — first press should
        // land on row 0, otherwise stay put.
        None => 0,
        Some(ix) => ix,
    };
    apply_cursor(this, &rows, new_ix);
}

/// On a header: collapse the group. On a leaf: jump to the parent
/// header (no auto-select — we keep the previously selected leaf so
/// the viewer doesn't reset on backwards-navigation).
fn on_select_left(this: &mut ExplorerApp) {
    let Some(focused) = this.tree_view.focused_row else {
        return;
    };
    match focused {
        FocusedRow::Header { group_ix } => {
            let ext = this.tree_view.groups[group_ix].ext;
            this.tree_view.expanded_groups.remove(ext);
        }
        FocusedRow::Leaf { group_ix, .. } => {
            this.tree_view.focused_row = Some(FocusedRow::Header { group_ix });
            scroll_to_focus(this);
        }
    }
}

/// On a header: expand the group. On a leaf: no-op (leaves have no
/// children in this tree).
fn on_select_right(this: &mut ExplorerApp) {
    let Some(focused) = this.tree_view.focused_row else {
        return;
    };
    if let FocusedRow::Header { group_ix } = focused {
        let ext = this.tree_view.groups[group_ix].ext;
        this.tree_view.expanded_groups.insert(ext);
    }
}

/// Map the current `focused_row` to its position in `rows`, falling
/// back to the row's group header when the leaf became invisible
/// (group collapsed externally).
fn current_row_index(this: &ExplorerApp, rows: &[TreeRow]) -> Option<usize> {
    let focused = this.tree_view.focused_row?;
    if let Some(ix) = this.tree_view.focused_row_index(focused) {
        return Some(ix);
    }
    // Leaf inside a now-collapsed group — clamp to its header.
    if let FocusedRow::Leaf { group_ix, .. } = focused {
        return rows
            .iter()
            .position(|r| matches!(r, TreeRow::Header { group_ix: g } if *g == group_ix));
    }
    None
}

/// Write the new cursor position into state, mirror leaf landings into
/// `selected_resource`, and scroll the row into view.
fn apply_cursor(this: &mut ExplorerApp, rows: &[TreeRow], new_ix: usize) {
    let row = rows[new_ix];
    let focused = FocusedRow::from_row(row);
    this.tree_view.focused_row = Some(focused);
    if let TreeRow::Leaf { group_ix, leaf_ix } = row {
        let idx = this.tree_view.groups[group_ix].entries[leaf_ix].idx;
        this.state.selected_resource = Some(idx);
    }
    this.tree_view
        .scroll_handle
        .scroll_to_item(new_ix, ScrollStrategy::Center);
}

/// Scroll just to the currently focused row (used by `Left` jumping
/// to a parent header).
fn scroll_to_focus(this: &mut ExplorerApp) {
    let Some(focused) = this.tree_view.focused_row else {
        return;
    };
    if let Some(ix) = this.tree_view.focused_row_index(focused) {
        this.tree_view
            .scroll_handle
            .scroll_to_item(ix, ScrollStrategy::Center);
    }
}

/// Row height in pixels. Hard-coded because `uniform_list` needs all
/// rows to share the same height — headers and leaves both fit easily
/// inside 24 px at our default 14-px body text.
const ROW_HEIGHT: f32 = 24.;

pub fn render(this: &mut ExplorerApp, cx: &mut Context<ExplorerApp>) -> impl IntoElement {
    let rows: Rc<Vec<TreeRow>> = Rc::new(this.tree_view.build_rows());
    let count = rows.len();
    let rows_for_closure = rows;
    let scroll_handle = this.tree_view.scroll_handle.clone();
    let focus_handle = this.tree_view.focus_handle.clone();

    let list = uniform_list(
        "key-file-tree",
        count,
        cx.processor(move |this, range: Range<usize>, _window, cx| {
            // Snapshot the colors we need before borrowing `cx`
            // mutably for `cx.listener` below. `Hsla` is `Copy`.
            let theme = cx.theme();
            let sidebar_fg = theme.sidebar_foreground;
            let sidebar_accent = theme.sidebar_accent;
            let accent_bg = theme.accent;
            let accent_fg = theme.accent_foreground;
            let transparent = theme.transparent;
            let radius = theme.radius;
            let ring = theme.ring;

            let mut items = Vec::with_capacity(range.len());
            for row_ix in range {
                let row = rows_for_closure[row_ix];
                let focused = this.tree_view.focused_row == Some(FocusedRow::from_row(row));
                let el = match row {
                    TreeRow::Header { group_ix } => {
                        let group = &this.tree_view.groups[group_ix];
                        let ext = group.ext;
                        let leaf_count = group.entries.len();
                        let is_open = this.tree_view.expanded_groups.contains(ext);
                        let chevron = if is_open { "▾" } else { "▸" };

                        let mut row = h_flex()
                            .id(("tree-row", row_ix))
                            .h(px(ROW_HEIGHT))
                            .px_1()
                            .gap_1()
                            .items_center()
                            .rounded(radius)
                            .text_color(sidebar_fg)
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .hover(|s| s.bg(sidebar_accent))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tree_view.focused_row =
                                    Some(FocusedRow::Header { group_ix });
                                if !this.tree_view.expanded_groups.remove(ext) {
                                    this.tree_view.expanded_groups.insert(ext);
                                }
                                cx.notify();
                            }))
                            .child(div().w(px(12.)).child(chevron))
                            .child(div().child(format!("{ext} ({leaf_count})")));
                        if focused {
                            row = row.border_l_2().border_color(ring);
                        }
                        row
                    }
                    TreeRow::Leaf { group_ix, leaf_ix } => {
                        let leaf = &this.tree_view.groups[group_ix].entries[leaf_ix];
                        let idx = leaf.idx;
                        let label = leaf.label.clone();
                        let selected = this.state.selected_resource == Some(idx);
                        let (bg, fg) = if selected {
                            (accent_bg, accent_fg)
                        } else {
                            (transparent, sidebar_fg)
                        };
                        let mut row = h_flex()
                            .id(("tree-row", row_ix))
                            .h(px(ROW_HEIGHT))
                            .pl_5()
                            .pr_1()
                            .items_center()
                            .rounded(radius)
                            .bg(bg)
                            .text_color(fg)
                            .cursor_pointer()
                            .hover(|s| s.bg(sidebar_accent))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.tree_view.focused_row =
                                    Some(FocusedRow::Leaf { group_ix, leaf_ix });
                                this.state.selected_resource = Some(idx);
                                cx.notify();
                            }))
                            .child(label);
                        if focused {
                            row = row.border_l_2().border_color(ring);
                        }
                        row
                    }
                };
                items.push(el);
            }
            items
        }),
    )
    .track_scroll(scroll_handle.clone())
    .with_sizing_behavior(ListSizingBehavior::Auto)
    .size_full();

    // Wrapper hosts the focus token, the key context, and the
    // gpui-component scrollbar layer.
    div()
        .id("key-file-tree-wrapper")
        .size_full()
        .relative()
        .key_context(KEY_FILE_TREE_CONTEXT)
        .track_focus(&focus_handle)
        .on_action(cx.listener(|this, _: &SelectUp, _window, cx| {
            on_select_up(this);
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &SelectDown, _window, cx| {
            on_select_down(this);
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &SelectLeft, _window, cx| {
            on_select_left(this);
            cx.notify();
        }))
        .on_action(cx.listener(|this, _: &SelectRight, _window, cx| {
            on_select_right(this);
            cx.notify();
        }))
        .child(list)
        .vertical_scrollbar(&scroll_handle)
}
