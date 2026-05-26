use infinitier_cre_resource::Cre;
use infinitier_fnt_resource::Fnt;
use infinitier_ids_resource::Ids;
use infinitier_ini_resource::Ini;
use infinitier_itm_resource::Itm;
use infinitier_sav_resource::Sav;
use infinitier_spl_resource::Spl;
use infinitier_tlk_resource::Tlk;
use infinitier_ttf_resource::Ttf;
use infinitier_two_da_resource::TwoDA;
use infinitier_wed_resource::Wed;

use bam::ImportedBam;
use bcs::ImportedBcs;
use gam::ImportedGam;
use image::ImportedImage;
use movie::MovieSource;
use sound::SoundDecoder;
use tis::ImportedTis;

pub mod bam;
pub mod bcs;
pub mod gam;
pub mod image;
pub mod movie;
pub mod sound;
pub mod tis;

#[derive(Debug)]
pub enum ImportedResource {
    Are,
    Bah,
    Bam(ImportedBam),
    Bcs(ImportedBcs),
    Bio,
    Chr,
    Chu,
    Cre(Box<Cre>),
    Dlg,
    Eff,
    Fnt(Fnt),
    Gam(Box<ImportedGam>),
    Glsl,
    Gui,
    Ids(Ids),
    /// Unified image wrapper for every raster format (BMP / PVRZ / MOS /
    /// PNG for now).
    Image(ImportedImage),
    Ini(Ini),
    Itm(Itm),
    Lua,
    Maze,
    Menu,
    Mus,
    Mve(MovieSource),
    Plt,
    Pro,
    Sav(Sav),
    /// ACM and WAV/WAVC resources both decode to PCM via the unified
    /// streaming [`SoundDecoder`].
    Sound(SoundDecoder),
    Spl(Spl),
    Sql,
    Src,
    Sto,
    Tga,
    /// Decoded TIS tileset — palette and PVRZ variants are both
    /// pre-rendered into 64×64 RGBA tile buffers so the viewer only
    /// has to composite, not decode.
    Tis(ImportedTis),
    Toh,
    Tot,
    Ttf(Ttf),
    Tlk(Tlk),
    TwoDA(TwoDA),
    Vef,
    Vvc,
    Wbm(MovieSource),
    Wed(Wed),
    Wfx,
    Wmp,
    Unknown(u16),
}
