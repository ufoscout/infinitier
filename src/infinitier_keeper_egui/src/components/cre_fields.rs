//! Per-version CRE-field dispatch for the bits the egui keeper
//! currently needs. Mirrors the `cre_fields` module on the GPUI
//! keeper but only ports the helpers actually consumed here — for
//! now, the portrait resref accessors used by [`crate::components::portraits`].

use infinitier_core::resource::cre::{Cre, CreHeader};

/// Name of the small-portrait resource (BMP). Stripped of trailing
/// NULs by the importer.
pub fn small_portrait_name(cre: &Cre) -> &str {
    match &cre.header {
        CreHeader::V10(h) => &h.small_portrait_bmp,
        CreHeader::V12(h) => &h.small_portrait_bmp,
        CreHeader::V90(h) => &h.small_portrait,
        CreHeader::V22(h) => &h.small_portrait_bmp,
    }
}

/// Name of the large-portrait resource. Usually a BMP. V10's field
/// is named `large_portrait_pstee_bam_other_games` because PSTEE
/// uses a BAM here while every other engine uses a BMP — the keeper
/// loads it as a BMP and falls back to the small portrait on failure.
pub fn large_portrait_name(cre: &Cre) -> &str {
    match &cre.header {
        CreHeader::V10(h) => &h.large_portrait_pstee_bam_other_games,
        CreHeader::V12(h) => &h.large_portrait_bmp,
        CreHeader::V90(h) => &h.large_portrait,
        CreHeader::V22(h) => &h.large_portrait_bmp,
    }
}
