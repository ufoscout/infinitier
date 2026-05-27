//! One Rust module per resource viewer. Each module exposes a
//! `populate(window, …)` that writes the relevant Slint properties
//! and sets `viewer-kind` to its discriminator.

pub mod bam;
pub mod bcs;
pub mod common;
pub mod fnt;
pub mod ids;
pub mod image;
pub mod ini;
pub mod message;
pub mod movie;
pub mod sound;
pub mod stub;
pub mod tis;
pub mod two_da;
