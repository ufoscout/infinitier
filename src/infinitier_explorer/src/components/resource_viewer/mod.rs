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
mod bmp;
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
mod ini;
mod itm;
mod lua;
mod maze;
mod menu;
mod mos;
mod movie;
mod mus;
mod plt;
mod png;
mod pro;
mod pvrz;
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
use bmp::BmpViewer;
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
use ini::IniViewer;
use itm::ItmViewer;
use lua::LuaViewer;
use maze::MazeViewer;
use menu::MenuViewer;
use mos::MosViewer;
use movie::MovieViewer;
use mus::MusViewer;
use plt::PltViewer;
use png::PngViewer;
use pro::ProViewer;
use pvrz::PvrzViewer;
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
                                ImportedResource::Bmp(bmp) => {
                                    Box::new(BmpViewer::new(bmp, ui, resource_id))
                                }
                                ImportedResource::Ids(ids) => Box::new(IdsViewer::new(ids)),
                                ImportedResource::Ini(ini) => Box::new(IniViewer::new(ini)),
                                ImportedResource::Pvrz(prvz) => Box::new(PvrzViewer::new(prvz)),
                                ImportedResource::TwoDA(twoda) => Box::new(TwoDAViewer::new(twoda)),
                                ImportedResource::Wed(wed) => Box::new(WedViewer::new(wed)),
                                ImportedResource::Sound(sd) => Box::new(SoundViewer::new(sd)),
                                ImportedResource::Are => Box::new(AreViewer::new()),
                                ImportedResource::Bah => Box::new(BahViewer::new()),
                                ImportedResource::Bcs(bcs) => Box::new(BcsViewer::new(bcs)),
                                ImportedResource::Bio => Box::new(BioViewer::new()),
                                ImportedResource::Chr => Box::new(ChrViewer::new()),
                                ImportedResource::Chu => Box::new(ChuViewer::new()),
                                ImportedResource::Cre => Box::new(CreViewer::new()),
                                ImportedResource::Dlg => Box::new(DlgViewer::new()),
                                ImportedResource::Eff => Box::new(EffViewer::new()),
                                ImportedResource::Fnt => Box::new(FntViewer::new()),
                                ImportedResource::Gam => Box::new(GamViewer::new()),
                                ImportedResource::Glsl => Box::new(GlslViewer::new()),
                                ImportedResource::Gui => Box::new(GuiViewer::new()),
                                ImportedResource::Itm => Box::new(ItmViewer::new()),
                                ImportedResource::Lua => Box::new(LuaViewer::new()),
                                ImportedResource::Maze => Box::new(MazeViewer::new()),
                                ImportedResource::Menu => Box::new(MenuViewer::new()),
                                ImportedResource::Mos => Box::new(MosViewer::new()),
                                ImportedResource::Mus => Box::new(MusViewer::new()),
                                ImportedResource::Mve(src) => Box::new(MovieViewer::new(src)),
                                ImportedResource::Plt => Box::new(PltViewer::new()),
                                ImportedResource::Png => Box::new(PngViewer::new()),
                                ImportedResource::Pro => Box::new(ProViewer::new()),
                                ImportedResource::Spl => Box::new(SplViewer::new()),
                                ImportedResource::Sql => Box::new(SqlViewer::new()),
                                ImportedResource::Src => Box::new(SrcViewer::new()),
                                ImportedResource::Sto => Box::new(StoViewer::new()),
                                ImportedResource::Tga => Box::new(TgaViewer::new()),
                                ImportedResource::Tis => Box::new(TisViewer::new()),
                                ImportedResource::Toh => Box::new(TohViewer::new()),
                                ImportedResource::Tot => Box::new(TotViewer::new()),
                                ImportedResource::Ttf => Box::new(TtfViewer::new()),
                                ImportedResource::Vef => Box::new(VefViewer::new()),
                                ImportedResource::Vvc => Box::new(VvcViewer::new()),
                                ImportedResource::Wbm(src) => Box::new(MovieViewer::new(src)),
                                ImportedResource::Wfx => Box::new(WfxViewer::new()),
                                ImportedResource::Wmp => Box::new(WmpViewer::new()),
                                ImportedResource::Unknown(_) => Box::new(UnknownViewer::new()),
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
