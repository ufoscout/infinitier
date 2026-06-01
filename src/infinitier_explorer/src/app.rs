use eframe::egui;
use infinitier_core::game;

use crate::state::AppState;
use crate::ui;

pub struct ExplorerApp {
    state: AppState,
    central_panel: ui::central_panel::CentralPanel,
    left_panel: ui::left_panel::LeftPanel,
    bottom_panel: ui::bottom_panel::BottomPanel,
}

impl ExplorerApp {
    pub fn new(game_data: game::GameData) -> Self {
        Self {
            central_panel: ui::central_panel::CentralPanel::new(),
            left_panel: ui::left_panel::LeftPanel::new(&game_data),
            bottom_panel: ui::bottom_panel::BottomPanel,
            state: AppState::new(game_data),
        }
    }
}

impl eframe::App for ExplorerApp {

        /// Pre-frame hook (runs once before [`Self::ui`], inside the egui
    /// pass). Works around egui issue https://github.com/emilk/egui/issues/2142: a focused text field only
    /// surrenders keyboard focus on Escape / Tab / arrow-nav or when
    /// another *focusable* widget is clicked — clicking empty space or a
    /// plain label leaves it focused forever, so `Response::lost_focus()`
    /// never fires and the commit-on-blur path in
    /// [`KeeperEditors::show_input`] never runs.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        surrender_focus_on_outside_click(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.bottom_panel.show(ui, &self.state);
        self.left_panel.show(ui, &mut self.state);
        self.central_panel.show(ui, &self.state);
    }
}


/// Drop keyboard focus from the currently-focused widget when the latest
/// pointer press landed outside it. See [`KeeperApp::logic`] for why this
/// is needed (egui https://github.com/emilk/egui/issues/2142) and why it has to run at the top of the frame.
///
/// Generic on purpose: any focused widget surrenders focus on an outside
/// click, which matches normal desktop behaviour and means every editable
/// row — present and future — gets correct commit-on-blur for free,
/// without each call site having to opt in.
fn surrender_focus_on_outside_click(ctx: &egui::Context) {
    // Only a fresh pointer press can move focus off a field.
    if !ctx.input(|i| i.pointer.any_pressed()) {
        return;
    }
    let Some(focused) = ctx.memory(|m| m.focused()) else {
        return;
    };
    let Some(press_pos) = ctx.input(|i| i.pointer.interact_pos()) else {
        return;
    };
    // Rect of the focused widget as laid out last frame — it sits in the
    // same place this frame, since `logic` runs before anything is
    // redrawn. A press inside it means the user is interacting with the
    // field itself (clicking to place the caret, selecting text), so leave
    // focus alone; only an outside press counts as a blur.
    let clicked_inside = ctx
        .read_response(focused)
        .is_some_and(|r| r.interact_rect.contains(press_pos));
    if !clicked_inside {
        ctx.memory_mut(|m| m.surrender_focus(focused));
    }
}