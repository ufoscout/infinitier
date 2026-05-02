use std::sync::Arc;

use gpui::RenderImage;
use infinitier_core::game::ResourceId;
use infinitier_core::resource::key::ResourceType;

use crate::state::AppState;

mod acm;
mod are;
mod bah;
mod bam;
mod bcs;
mod bio;
mod bmp;
mod bs;
mod chr;
mod chu;
mod cre;
mod dlg;
mod eff;
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
mod mve;
mod mus;
mod plt;
mod png;
mod pro;
mod pvrz;
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
mod wav;
mod wbm;
mod wed;
mod wfx;
mod wmp;

use acm::AcmViewer;
use are::AreViewer;
use bah::BahViewer;
use bam::BamViewer;
use bcs::BcsViewer;
use bio::BioViewer;
use bmp::BmpViewer;
use bs::BsViewer;
use chr::ChrViewer;
use chu::ChuViewer;
use cre::CreViewer;
use dlg::DlgViewer;
use eff::EffViewer;
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
use mve::MveViewer;
use mus::MusViewer;
use plt::PltViewer;
use png::PngViewer;
use pro::ProViewer;
use pvrz::PvrzViewer;
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
use wav::WavViewer;
use wbm::WbmViewer;
use wed::WedViewer;
use wfx::WfxViewer;
use wmp::WmpViewer;

pub enum ViewerData {
    Text(String),
    Image(Arc<RenderImage>),
}

pub struct ResourceViewer;

impl ResourceViewer {
    pub fn get_data(state: &AppState, resource_id: ResourceId) -> ViewerData {
        match state.game_data.get_by_id(resource_id) {
            None => ViewerData::Text("Resource not found.".to_string()),
            Some(resource) => match resource.r#type {
                ResourceType::Acm => ViewerData::Text(AcmViewer::label().to_string()),
                ResourceType::Are => ViewerData::Text(AreViewer::label().to_string()),
                ResourceType::Bah => ViewerData::Text(BahViewer::label().to_string()),
                ResourceType::Bam => ViewerData::Text(BamViewer::label().to_string()),
                ResourceType::Bcs => ViewerData::Text(BcsViewer::label().to_string()),
                ResourceType::Bio => ViewerData::Text(BioViewer::label().to_string()),
                ResourceType::Bmp => BmpViewer::data(state, resource_id),
                ResourceType::Bs => ViewerData::Text(BsViewer::label().to_string()),
                ResourceType::Chr => ViewerData::Text(ChrViewer::label().to_string()),
                ResourceType::Chu => ViewerData::Text(ChuViewer::label().to_string()),
                ResourceType::Cre => ViewerData::Text(CreViewer::label().to_string()),
                ResourceType::Dlg => ViewerData::Text(DlgViewer::label().to_string()),
                ResourceType::Eff => ViewerData::Text(EffViewer::label().to_string()),
                ResourceType::Fnt => ViewerData::Text(FntViewer::label().to_string()),
                ResourceType::Gam => ViewerData::Text(GamViewer::label().to_string()),
                ResourceType::Glsl => ViewerData::Text(GlslViewer::label().to_string()),
                ResourceType::Gui => ViewerData::Text(GuiViewer::label().to_string()),
                ResourceType::Ids => ViewerData::Text(IdsViewer::label().to_string()),
                ResourceType::Ini => ViewerData::Text(IniViewer::label().to_string()),
                ResourceType::Itm => ViewerData::Text(ItmViewer::label().to_string()),
                ResourceType::Lua => ViewerData::Text(LuaViewer::label().to_string()),
                ResourceType::Maze => ViewerData::Text(MazeViewer::label().to_string()),
                ResourceType::Menu => ViewerData::Text(MenuViewer::label().to_string()),
                ResourceType::Mos => ViewerData::Text(MosViewer::label().to_string()),
                ResourceType::Mve => ViewerData::Text(MveViewer::label().to_string()),
                ResourceType::Mus => ViewerData::Text(MusViewer::label().to_string()),
                ResourceType::Plt => ViewerData::Text(PltViewer::label().to_string()),
                ResourceType::Png => ViewerData::Text(PngViewer::label().to_string()),
                ResourceType::Pro => ViewerData::Text(ProViewer::label().to_string()),
                ResourceType::Pvrz => ViewerData::Text(PvrzViewer::label().to_string()),
                ResourceType::Spl => ViewerData::Text(SplViewer::label().to_string()),
                ResourceType::Sql => ViewerData::Text(SqlViewer::label().to_string()),
                ResourceType::Src => ViewerData::Text(SrcViewer::label().to_string()),
                ResourceType::Sto => ViewerData::Text(StoViewer::label().to_string()),
                ResourceType::Tga => ViewerData::Text(TgaViewer::label().to_string()),
                ResourceType::Tis => ViewerData::Text(TisViewer::label().to_string()),
                ResourceType::Toh => ViewerData::Text(TohViewer::label().to_string()),
                ResourceType::Tot => ViewerData::Text(TotViewer::label().to_string()),
                ResourceType::Ttf => ViewerData::Text(TtfViewer::label().to_string()),
                ResourceType::TwoDA => ViewerData::Text(TwoDAViewer::label().to_string()),
                ResourceType::Vef => ViewerData::Text(VefViewer::label().to_string()),
                ResourceType::Vvc => ViewerData::Text(VvcViewer::label().to_string()),
                ResourceType::Wav => ViewerData::Text(WavViewer::label().to_string()),
                ResourceType::Wbm => ViewerData::Text(WbmViewer::label().to_string()),
                ResourceType::Wed => ViewerData::Text(WedViewer::label().to_string()),
                ResourceType::Wfx => ViewerData::Text(WfxViewer::label().to_string()),
                ResourceType::Wmp => ViewerData::Text(WmpViewer::label().to_string()),
                ResourceType::Unknown(type_id) => {
                    ViewerData::Text(UnknownViewer::label(type_id))
                }
            },
        }
    }
}
