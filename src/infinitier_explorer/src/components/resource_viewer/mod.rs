use eframe::egui;
use infinitier_core::{
    game::{GameResource, ResourceId},
    imported_resource::ImportedResource,
};
use log::*;

use crate::state::AppState;

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

pub trait ResourceViewerTrait {
    fn show(&mut self, ui: &mut egui::Ui, resource_id: ResourceId, resource: &GameResource);
}

pub struct ResourceViewer {
    inner: Option<InnerResource>,
}

struct InnerResource {
    id: ResourceId,
    resource: Box<dyn ResourceViewerTrait>,
}

impl ResourceViewer {
    pub fn new() -> Self {
        Self { inner: None }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, state: &AppState) {
        match state.selected_resource {
            None => {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a resource from the panel on the left.");
                });
            }
            Some(resource_id) => {
                if let Some(inner) = &mut self.inner
                    && inner.id == resource_id
                {
                    inner.resource.show(
                        ui,
                        resource_id,
                        state.game_data.get_by_id(resource_id).unwrap(),
                    );
                    return;
                }
                let viewer: Box<dyn ResourceViewerTrait> =
                    if let Some(resource) = state.game_data.get_by_id(resource_id) {
                        match resource.import(&state.game_data) {
                            Ok(imported) => match imported {
                                ImportedResource::Bam(bam) => {
                                    Box::new(BamViewer::new(bam, ui, resource_id))
                                }
                                ImportedResource::Ids(ids) => Box::new(IdsViewer::new(ids)),
                                ImportedResource::Image(img) => {
                                    Box::new(ImageViewer::new(img, ui, resource_id))
                                }
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
                                ImportedResource::Tis(tis) => {
                                    Box::new(TisViewer::new(tis, ui, resource_id))
                                }
                                ImportedResource::Toh => Box::new(TohViewer::new()),
                                ImportedResource::Tot => Box::new(TotViewer::new()),
                                ImportedResource::Ttf(ttf) => {
                                    Box::new(TtfViewer::new(ttf, ui, resource_id))
                                }
                                ImportedResource::Vef => Box::new(VefViewer::new()),
                                ImportedResource::Vvc => Box::new(VvcViewer::new()),
                                ImportedResource::Wbm(src) => Box::new(MovieViewer::new(src)),
                                ImportedResource::Wfx => Box::new(WfxViewer::new()),
                                ImportedResource::Wmp => Box::new(WmpViewer::new()),
                                // Sav / Tlk have no `ResourceType` entry yet,
                                // so the import dispatcher in
                                // `infinitier_core::game` never produces them
                                // here. Treat as Unknown for safety until they
                                // get dedicated viewers.
                                ImportedResource::Sav(_)
                                | ImportedResource::Tlk(_)
                                | ImportedResource::Unknown(_) => Box::new(UnknownViewer::new()),
                            },
                            Err(err) => {
                                error!("Error importing resource: {resource_id:?}, {err:?}");
                                Box::new(ErrorViewer::new(format!(
                                    "Error importing resource: {resource_id:?}, {err:?}"
                                )))
                            }
                        }
                    } else {
                        error!("Resource not found: {resource_id:?}");
                        Box::new(ErrorViewer::new(format!(
                            "Resource not found: {resource_id:?}"
                        )))
                    };

                self.inner = Some(InnerResource {
                    id: resource_id,
                    resource: viewer,
                });
                if let Some(inner) = &mut self.inner
                    && let Some(resource) = state.game_data.get_by_id(resource_id)
                {
                    inner.resource.show(ui, resource_id, resource);
                }
            }
        }
    }
}
