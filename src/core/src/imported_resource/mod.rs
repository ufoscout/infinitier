use infinitier_ids_resource::Ids;
use infinitier_ini_resource::Ini;
use infinitier_ttf_resource::Ttf;
use infinitier_two_da_resource::TwoDA;
use infinitier_wed_resource::Wed;

use bam::ImportedBam;
use bcs::ImportedBcs;
use image::ImportedImage;
use movie::MovieSource;
use sound::SoundDecoder;
use tis::ImportedTis;

pub mod bam;
pub mod bcs;
pub mod image;
pub mod movie;
pub mod sound;
pub mod tis;

#[derive(Debug)]
pub enum ImportedResource {
    // Types with importers
    Bam(ImportedBam),
    Bcs(ImportedBcs),
    Ids(Ids),
    /// Unified image wrapper for every raster format (BMP / PVRZ / MOS /
    /// PNG for now; TGA / TIS later).
    Image(ImportedImage),
    Ini(Ini),
    /// ACM and WAV/WAVC resources both decode to PCM via the unified
    /// streaming [`SoundDecoder`].
    Sound(SoundDecoder),
    /// Decoded TIS tileset — palette and PVRZ variants are both
    /// pre-rendered into 64×64 RGBA tile buffers so the viewer only
    /// has to composite, not decode.
    Tis(ImportedTis),
    TwoDA(TwoDA),
    Wed(Wed),
    // Types without importers
    Are,
    Bah,
    Bio,
    Chr,
    Chu,
    Cre,
    Dlg,
    Eff,
    Fnt,
    Gam,
    Glsl,
    Gui,
    Itm,
    Lua,
    Maze,
    Menu,
    Mus,
    Mve(MovieSource),
    Plt,
    Pro,
    Spl,
    Sql,
    Src,
    Sto,
    Tga,
    Toh,
    Tot,
    /// Parsed TrueType / OpenType font — name table + em metrics for
    /// the info bar, plus a shared handle to the raw bytes so the
    /// viewer can install the font into egui's text renderer for
    /// sample-text rendering.
    Ttf(Ttf),
    Vef,
    Vvc,
    Wbm(MovieSource),
    Wfx,
    Wmp,
    Unknown(u16),
}
