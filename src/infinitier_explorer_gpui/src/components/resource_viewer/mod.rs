//! Dispatcher to per-resource-type viewer modules. Mirrors the egui
//! `ResourceViewer` cache: the inner `Box<dyn ResourceViewerTrait>` is
//! rebuilt only when the selected resource changes, so the import
//! step (which can be expensive — BAM decode, image decode, etc.) only
//! happens once per selection. Every viewer in this port is currently
//! a stub matching the egui version, so structural parity is the goal.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use infinitier_core::{
    game::{GameResource, ResourceId},
    imported_resource::ImportedResource,
};
use log::*;

use crate::app::ExplorerApp;

mod are;
mod baf_highlight;
mod bah;
mod bam;
mod bcs;
mod bio;
mod chr;
mod chu;
mod cre;
mod dlg;
mod eff;
mod error;
mod fnt;
mod gam;
mod glsl;
mod gui;
mod ids;
mod image;
mod ini;
mod itm;
mod lua;
mod maze;
mod menu;
mod movie;
mod mus;
mod plt;
mod pro;
mod sound;
mod spl;
mod sql;
mod src;
mod sto;
mod tga;
mod tis;
mod toh;
mod tot;
mod ttf;
mod two_da;
mod unknown;
mod vef;
mod vvc;
mod wed;
mod wfx;
mod wmp;

use are::AreViewer;
use bah::BahViewer;
use bam::BamViewer;
use bcs::BcsViewer;
use bio::BioViewer;
use chr::ChrViewer;
use chu::ChuViewer;
use cre::CreViewer;
use dlg::DlgViewer;
use eff::EffViewer;
use error::ErrorViewer;
use fnt::FntViewer;
use gam::GamViewer;
use glsl::GlslViewer;
use gui::GuiViewer;
use ids::IdsViewer;
use image::ImageViewer;
use ini::IniViewer;
use itm::ItmViewer;
use lua::LuaViewer;
use maze::MazeViewer;
use menu::MenuViewer;
use movie::MovieViewer;
use mus::MusViewer;
use plt::PltViewer;
use pro::ProViewer;
use sound::SoundViewer;
use spl::SplViewer;
use sql::SqlViewer;
use src::SrcViewer;
use sto::StoViewer;
use tga::TgaViewer;
use tis::TisViewer;
use toh::TohViewer;
use tot::TotViewer;
use ttf::TtfViewer;
use two_da::TwoDAViewer;
use unknown::UnknownViewer;
use vef::VefViewer;
use vvc::VvcViewer;
use wed::WedViewer;
use wfx::WfxViewer;
use wmp::WmpViewer;

/// Common interface every per-type viewer implements. The egui port
/// uses `&mut egui::Ui`; here we return an `AnyElement` so the
/// surrounding panel can stitch us into its layout. `&mut self` is
/// kept so future viewers (BAM playback, image cache, scroll position)
/// can carry mutable state across frames the same way the egui port
/// already does.
/// Extends `Any` so click handlers in concrete viewers can downcast
/// `Box<dyn ResourceViewerTrait>` back to themselves (Rust 1.86+
/// trait upcasting). The `BamViewer` controls use this so their
/// `cx.listener` closures, which only get `&mut ExplorerApp`, can
/// reach into the cached viewer state.
pub trait ResourceViewerTrait: std::any::Any {
    fn render(
        &mut self,
        resource_id: ResourceId,
        resource: &GameResource,
        window: &mut Window,
        cx: &mut Context<ExplorerApp>,
    ) -> AnyElement;
}

pub struct ResourceViewer {
    inner: Option<InnerResource>,
}

struct InnerResource {
    id: ResourceId,
    viewer: Box<dyn ResourceViewerTrait>,
}

impl ResourceViewer {
    pub fn new() -> Self {
        Self { inner: None }
    }
}

/// Render entry point used by `central_panel::render`. Borrows the
/// app mutably so we can lazily (re)build the cached viewer. Returns
/// a generic placeholder when nothing is selected.
pub fn render(
    this: &mut ExplorerApp,
    window: &mut Window,
    cx: &mut Context<ExplorerApp>,
) -> AnyElement {
    let Some(resource_id) = this.state.selected_resource else {
        return div()
            .w_full()
            .p_6()
            .child("Select a resource from the panel on the left.")
            .into_any_element();
    };

    // Cache miss: rebuild the inner viewer. Done before the render
    // call so the resource borrow doesn't conflict with the mutable
    // viewer borrow below.
    let cache_hit = this
        .viewer
        .inner
        .as_ref()
        .map(|i| i.id == resource_id)
        .unwrap_or(false);
    if !cache_hit {
        let viewer = match this.state.game_data.get_by_id(resource_id) {
            Some(resource) => match resource.import(&this.state.game_data) {
                Ok(imported) => build_viewer(imported),
                Err(err) => {
                    error!("Error importing resource: {resource_id:?}, {err:?}");
                    Box::new(ErrorViewer::new(format!(
                        "Error importing resource: {resource_id:?}, {err:?}"
                    ))) as Box<dyn ResourceViewerTrait>
                }
            },
            None => {
                error!("Resource not found: {resource_id:?}");
                Box::new(ErrorViewer::new(format!(
                    "Resource not found: {resource_id:?}"
                )))
            }
        };
        this.viewer.inner = Some(InnerResource {
            id: resource_id,
            viewer,
        });
    }

    // `state.game_data` and `viewer.inner` are disjoint fields, so
    // the borrow checker is happy to lend us the resource immutably
    // while the viewer cache is borrowed mutably.
    let Some(resource) = this.state.game_data.get_by_id(resource_id) else {
        return div()
            .w_full()
            .p_6()
            .child(format!("Resource not found: {resource_id:?}"))
            .into_any_element();
    };
    let inner = this
        .viewer
        .inner
        .as_mut()
        .expect("inner populated above on cache miss");
    inner.viewer.render(resource_id, resource, window, cx)
}

/// Map an `ImportedResource` variant to the right viewer struct.
/// Kept in a free function so `render` reads top-to-bottom.
fn build_viewer(imported: ImportedResource) -> Box<dyn ResourceViewerTrait> {
    match imported {
        ImportedResource::Bam(bam) => Box::new(BamViewer::new(bam)),
        ImportedResource::Ids(ids) => Box::new(IdsViewer::new(ids)),
        ImportedResource::Image(img) => Box::new(ImageViewer::new(img)),
        ImportedResource::Ini(ini) => Box::new(IniViewer::new(ini)),
        ImportedResource::TwoDA(twoda) => Box::new(TwoDAViewer::new(twoda)),
        ImportedResource::Wed(wed) => Box::new(WedViewer::new(wed)),
        ImportedResource::Sound(sd) => Box::new(SoundViewer::new(sd)),
        ImportedResource::Are => Box::new(AreViewer::new()),
        ImportedResource::Bah => Box::new(BahViewer::new()),
        ImportedResource::Bcs(bcs) => Box::new(BcsViewer::new(bcs)),
        ImportedResource::Bio => Box::new(BioViewer::new()),
        ImportedResource::Chr => Box::new(ChrViewer::new()),
        ImportedResource::Chu => Box::new(ChuViewer::new()),
        ImportedResource::Cre(_) => Box::new(CreViewer::new()),
        ImportedResource::Dlg => Box::new(DlgViewer::new()),
        ImportedResource::Eff => Box::new(EffViewer::new()),
        ImportedResource::Fnt(fnt) => Box::new(FntViewer::new(fnt)),
        ImportedResource::Gam(_) => Box::new(GamViewer::new()),
        ImportedResource::Glsl => Box::new(GlslViewer::new()),
        ImportedResource::Gui => Box::new(GuiViewer::new()),
        ImportedResource::Itm(_) => Box::new(ItmViewer::new()),
        ImportedResource::Lua => Box::new(LuaViewer::new()),
        ImportedResource::Maze => Box::new(MazeViewer::new()),
        ImportedResource::Menu => Box::new(MenuViewer::new()),
        ImportedResource::Mus => Box::new(MusViewer::new()),
        ImportedResource::Mve(src) => Box::new(MovieViewer::new(src)),
        ImportedResource::Plt => Box::new(PltViewer::new()),
        ImportedResource::Pro => Box::new(ProViewer::new()),
        ImportedResource::Spl(_) => Box::new(SplViewer::new()),
        ImportedResource::Sql => Box::new(SqlViewer::new()),
        ImportedResource::Src => Box::new(SrcViewer::new()),
        ImportedResource::Sto => Box::new(StoViewer::new()),
        ImportedResource::Tga => Box::new(TgaViewer::new()),
        ImportedResource::Tis(tis) => Box::new(TisViewer::new(tis)),
        ImportedResource::Toh => Box::new(TohViewer::new()),
        ImportedResource::Tot => Box::new(TotViewer::new()),
        ImportedResource::Ttf(ttf) => Box::new(TtfViewer::new(ttf)),
        ImportedResource::Vef => Box::new(VefViewer::new()),
        ImportedResource::Vvc => Box::new(VvcViewer::new()),
        ImportedResource::Wbm(src) => Box::new(MovieViewer::new(src)),
        ImportedResource::Wfx => Box::new(WfxViewer::new()),
        ImportedResource::Wmp => Box::new(WmpViewer::new()),
        // Sav / Tlk have no `ResourceType` entry yet, so the import
        // dispatcher in `infinitier_core::game` never produces them
        // here. Treat as Unknown until they get dedicated viewers.
        ImportedResource::Sav(_)
        | ImportedResource::Tlk(_)
        | ImportedResource::Unknown(_) => Box::new(UnknownViewer::new()),
    }
}

/// Render helper used by every stub viewer — a centred label with
/// the type name. Keeps each stub one-liner.
pub(crate) fn label(text: impl Into<gpui::SharedString>) -> AnyElement {
    div().w_full().p_2().child(text.into()).into_any_element()
}
