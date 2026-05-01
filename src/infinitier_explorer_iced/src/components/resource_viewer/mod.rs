use iced::widget::container;
use iced::{Element, Length};

use infinitier_core::resource::key::ResourceType;

use crate::state::{AppState, Message};

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

pub struct ResourceViewer;

impl ResourceViewer {
    pub fn view<'a>(&self, state: &'a AppState) -> Element<'a, Message> {
        let content: Element<'a, Message> = match state.selected {
            None => {
                iced::widget::text("Select a resource from the panel on the left.").into()
            }
            Some(resource_id) => match state.game_data.get_by_id(resource_id) {
                None => iced::widget::text("Resource not found.").into(),
                Some(resource) => match resource.r#type {
                    ResourceType::Acm => AcmViewer::view(),
                    ResourceType::Are => AreViewer::view(),
                    ResourceType::Bah => BahViewer::view(),
                    ResourceType::Bam => BamViewer::view(),
                    ResourceType::Bcs => BcsViewer::view(),
                    ResourceType::Bio => BioViewer::view(),
                    ResourceType::Bmp => BmpViewer::view(state, resource_id),
                    ResourceType::Bs => BsViewer::view(),
                    ResourceType::Chr => ChrViewer::view(),
                    ResourceType::Chu => ChuViewer::view(),
                    ResourceType::Cre => CreViewer::view(),
                    ResourceType::Dlg => DlgViewer::view(),
                    ResourceType::Eff => EffViewer::view(),
                    ResourceType::Fnt => FntViewer::view(),
                    ResourceType::Gam => GamViewer::view(),
                    ResourceType::Glsl => GlslViewer::view(),
                    ResourceType::Gui => GuiViewer::view(),
                    ResourceType::Ids => IdsViewer::view(),
                    ResourceType::Ini => IniViewer::view(),
                    ResourceType::Itm => ItmViewer::view(),
                    ResourceType::Lua => LuaViewer::view(),
                    ResourceType::Maze => MazeViewer::view(),
                    ResourceType::Menu => MenuViewer::view(),
                    ResourceType::Mos => MosViewer::view(),
                    ResourceType::Mve => MveViewer::view(),
                    ResourceType::Mus => MusViewer::view(),
                    ResourceType::Plt => PltViewer::view(),
                    ResourceType::Png => PngViewer::view(),
                    ResourceType::Pro => ProViewer::view(),
                    ResourceType::Pvrz => PvrzViewer::view(),
                    ResourceType::Spl => SplViewer::view(),
                    ResourceType::Sql => SqlViewer::view(),
                    ResourceType::Src => SrcViewer::view(),
                    ResourceType::Sto => StoViewer::view(),
                    ResourceType::Tga => TgaViewer::view(),
                    ResourceType::Tis => TisViewer::view(),
                    ResourceType::Toh => TohViewer::view(),
                    ResourceType::Tot => TotViewer::view(),
                    ResourceType::Ttf => TtfViewer::view(),
                    ResourceType::TwoDA => TwoDAViewer::view(),
                    ResourceType::Vef => VefViewer::view(),
                    ResourceType::Vvc => VvcViewer::view(),
                    ResourceType::Wav => WavViewer::view(),
                    ResourceType::Wbm => WbmViewer::view(),
                    ResourceType::Wed => WedViewer::view(),
                    ResourceType::Wfx => WfxViewer::view(),
                    ResourceType::Wmp => WmpViewer::view(),
                    ResourceType::Unknown(type_id) => UnknownViewer::view(type_id),
                },
            },
        };

        container(content).center(Length::Fill).into()
    }
}
