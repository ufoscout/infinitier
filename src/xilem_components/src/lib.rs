//! Minimal, theme-driven Xilem component set for `infinitier_keeper_xilem`.
//!
//! Scope is deliberately tiny: only the primitives the keeper port uses,
//! modelled loosely on `egui_components`. Every constructor takes a
//! [`Theme`] and applies it through Xilem's `Style` properties, so the
//! visual design is a one-place change in a later step.

pub mod theme;
pub mod view;

pub use theme::Theme;
