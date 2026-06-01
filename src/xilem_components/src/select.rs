//! A combo/select component wrapping Masonry's `Selector` widget (which
//! pops up a list of options on click — a combo box). Xilem ships no
//! view for it, so this is a hand-written [`View`] modelled on the
//! built-in `slider` view, plus a themed [`select`] constructor for
//! parity with the rest of `xilem_components`.

use std::marker::PhantomData;

use xilem::core::{MessageCtx, MessageResult, Mut, View, ViewMarker};
use xilem::masonry::widgets::{self, SelectionChanged};
use xilem::{Pod, ViewCtx, WidgetView};

use crate::theme::Theme;

/// Themed combo box: shows `options[selected]`, opens the list on click,
/// and calls `on_change(state, picked_index)` when the selection changes.
///
/// The dropdown currently renders with Masonry's own styling; the `theme`
/// is accepted for API consistency and future theming.
pub fn select<State: 'static, Action: 'static, F>(
    theme: Theme,
    options: Vec<String>,
    selected: usize,
    on_change: F,
) -> impl WidgetView<State, Action>
where
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    let _ = theme;
    SelectView {
        options,
        selected,
        on_change,
        phantom: PhantomData,
    }
}

/// The [`View`] created by [`select`].
#[must_use = "View values do nothing unless provided to Xilem."]
struct SelectView<State, Action, F> {
    options: Vec<String>,
    selected: usize,
    on_change: F,
    phantom: PhantomData<fn(State) -> Action>,
}

impl<State, Action, F> SelectView<State, Action, F> {
    /// Clamp the requested selection into range (the widget debug-panics
    /// on an out-of-bounds index).
    fn clamped(&self) -> usize {
        self.selected.min(self.options.len().saturating_sub(1))
    }
}

impl<State, Action, F> ViewMarker for SelectView<State, Action, F> {}

impl<F, State, Action> View<State, Action, ViewCtx> for SelectView<State, Action, F>
where
    State: 'static,
    Action: 'static,
    F: Fn(&mut State, usize) -> Action + Send + Sync + 'static,
{
    type Element = Pod<widgets::Selector>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _: &mut State) -> (Self::Element, Self::ViewState) {
        (
            ctx.with_action_widget(|ctx| {
                let widget =
                    widgets::Selector::new(self.options.clone()).with_selected_option(self.clamped());
                ctx.create_pod(widget)
            }),
            (),
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        (): &mut Self::ViewState,
        _: &mut ViewCtx,
        mut element: Mut<'_, Self::Element>,
        _: &mut State,
    ) {
        if prev.options != self.options {
            // `set_options` resets the selection, so re-apply it below.
            widgets::Selector::set_options(&mut element, self.options.clone());
        }
        if prev.selected != self.selected || prev.options != self.options {
            widgets::Selector::select_option(&mut element, self.clamped());
        }
    }

    fn teardown(
        &self,
        (): &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'_, Self::Element>,
    ) {
        ctx.teardown_action_source(element);
    }

    fn message(
        &self,
        (): &mut Self::ViewState,
        message: &mut MessageCtx,
        _: Mut<'_, Self::Element>,
        app_state: &mut State,
    ) -> MessageResult<Action> {
        if message.take_first().is_some() {
            return MessageResult::Stale;
        }
        match message.take_message::<SelectionChanged>() {
            Some(change) => MessageResult::Action((self.on_change)(app_state, change.index)),
            None => MessageResult::Stale,
        }
    }
}
