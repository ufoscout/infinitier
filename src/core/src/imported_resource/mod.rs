use infinitier_ids_importer::Ids;
use infinitier_ini_importer::Ini;
use infinitier_two_da_importer::TwoDA;
use infinitier_wed_importer::Wed;

use bam::ImportedBam;
use bcs::ImportedBcs;
use image::ImportedImage;
use movie::MovieSource;
use sound::SoundDecoder;

pub mod bam;
pub mod bcs;
pub mod image;
pub mod movie;
pub mod sound;

#[derive(Debug)]
pub enum ImportedResource {
    // Types with importers
    Bam(ImportedBam),
    Bcs(ImportedBcs),
    Ids(Ids),
    /// Unified image wrapper for every raster format (BMP / PVRZ for
    /// now; TGA / PNG / MOS / TIS later).
    Image(ImportedImage),
    Ini(Ini),
    /// ACM and WAV/WAVC resources both decode to PCM via the unified
    /// streaming [`SoundDecoder`].
    Sound(SoundDecoder),
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
    Mos,
    Mus,
    Mve(MovieSource),
    Plt,
    Png,
    Pro,
    Spl,
    Sql,
    Src,
    Sto,
    Tga,
    Tis,
    Toh,
    Tot,
    Ttf,
    Vef,
    Vvc,
    Wbm(MovieSource),
    Wfx,
    Wmp,
    Unknown(u16),
}
