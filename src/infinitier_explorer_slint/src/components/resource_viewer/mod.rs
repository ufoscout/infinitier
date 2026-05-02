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
    Text(slint::SharedString),
    Image(slint::Image),
}

pub struct ResourceViewer;

impl ResourceViewer {
    pub fn get_data(state: &AppState, resource_id: ResourceId) -> ViewerData {
        match state.game_data.get_by_id(resource_id) {
            None => ViewerData::Text("Resource not found.".into()),
            Some(resource) => match resource.r#type {
                ResourceType::Acm => ViewerData::Text(AcmViewer::label().into()),
                ResourceType::Are => ViewerData::Text(AreViewer::label().into()),
                ResourceType::Bah => ViewerData::Text(BahViewer::label().into()),
                ResourceType::Bam => ViewerData::Text(BamViewer::label().into()),
                ResourceType::Bcs => ViewerData::Text(BcsViewer::label().into()),
                ResourceType::Bio => ViewerData::Text(BioViewer::label().into()),
                ResourceType::Bmp => BmpViewer::data(state, resource_id),
                ResourceType::Bs => ViewerData::Text(BsViewer::label().into()),
                ResourceType::Chr => ViewerData::Text(ChrViewer::label().into()),
                ResourceType::Chu => ViewerData::Text(ChuViewer::label().into()),
                ResourceType::Cre => ViewerData::Text(CreViewer::label().into()),
                ResourceType::Dlg => ViewerData::Text(DlgViewer::label().into()),
                ResourceType::Eff => ViewerData::Text(EffViewer::label().into()),
                ResourceType::Fnt => ViewerData::Text(FntViewer::label().into()),
                ResourceType::Gam => ViewerData::Text(GamViewer::label().into()),
                ResourceType::Glsl => ViewerData::Text(GlslViewer::label().into()),
                ResourceType::Gui => ViewerData::Text(GuiViewer::label().into()),
                ResourceType::Ids => ViewerData::Text(IdsViewer::label().into()),
                ResourceType::Ini => ViewerData::Text(IniViewer::label().into()),
                ResourceType::Itm => ViewerData::Text(ItmViewer::label().into()),
                ResourceType::Lua => ViewerData::Text(LuaViewer::label().into()),
                ResourceType::Maze => ViewerData::Text(MazeViewer::label().into()),
                ResourceType::Menu => ViewerData::Text(MenuViewer::label().into()),
                ResourceType::Mos => ViewerData::Text(MosViewer::label().into()),
                ResourceType::Mve => ViewerData::Text(MveViewer::label().into()),
                ResourceType::Mus => ViewerData::Text(MusViewer::label().into()),
                ResourceType::Plt => ViewerData::Text(PltViewer::label().into()),
                ResourceType::Png => ViewerData::Text(PngViewer::label().into()),
                ResourceType::Pro => ViewerData::Text(ProViewer::label().into()),
                ResourceType::Pvrz => ViewerData::Text(PvrzViewer::label().into()),
                ResourceType::Spl => ViewerData::Text(SplViewer::label().into()),
                ResourceType::Sql => ViewerData::Text(SqlViewer::label().into()),
                ResourceType::Src => ViewerData::Text(SrcViewer::label().into()),
                ResourceType::Sto => ViewerData::Text(StoViewer::label().into()),
                ResourceType::Tga => ViewerData::Text(TgaViewer::label().into()),
                ResourceType::Tis => ViewerData::Text(TisViewer::label().into()),
                ResourceType::Toh => ViewerData::Text(TohViewer::label().into()),
                ResourceType::Tot => ViewerData::Text(TotViewer::label().into()),
                ResourceType::Ttf => ViewerData::Text(TtfViewer::label().into()),
                ResourceType::TwoDA => ViewerData::Text(TwoDAViewer::label().into()),
                ResourceType::Vef => ViewerData::Text(VefViewer::label().into()),
                ResourceType::Vvc => ViewerData::Text(VvcViewer::label().into()),
                ResourceType::Wav => ViewerData::Text(WavViewer::label().into()),
                ResourceType::Wbm => ViewerData::Text(WbmViewer::label().into()),
                ResourceType::Wed => ViewerData::Text(WedViewer::label().into()),
                ResourceType::Wfx => ViewerData::Text(WfxViewer::label().into()),
                ResourceType::Wmp => ViewerData::Text(WmpViewer::label().into()),
                ResourceType::Unknown(type_id) => {
                    ViewerData::Text(UnknownViewer::label(type_id).into())
                }
            },
        }
    }
}
