//! Centred-message placeholder. Used for the "not implemented yet"
//! tabs and for the empty / external-CRE / empty-slot fallbacks.

use gpui::{IntoElement, ParentElement, SharedString, Styled, div};

pub fn render(text: impl Into<SharedString>) -> impl IntoElement {
    div().w_full().p_6().child(text.into())
}
