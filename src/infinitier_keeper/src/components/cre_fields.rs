//! Party-wide accessors on the loaded GAM.
//!
//! The per-creature getters/setters that used to live here now live on
//! [`infinitier_core::resource::cre::Cre`] as methods (`cre.ac_natural()`,
//! `cre.set_lore(v)`, …) and are unit-tested there, next to the format
//! they describe. What remains are the GAM-side fields the editor
//! exposes that aren't part of any single creature: party gold and
//! reputation.

use infinitier_core::imported_resource::gam::ImportedGam;
use infinitier_core::resource::gam::{
    Bg2GamData, BgGamData, EeGamData, GamEngineData, Iwd2GamData, IwdGamData, PstGamData,
};

/// Party gold, on the GAM header (u32). Same field name + type
/// across every engine.
pub fn party_gold(gam: &ImportedGam) -> u32 {
    gam.header.party_gold
}

pub fn set_party_gold(gam: &mut ImportedGam, value: u32) {
    gam.header.party_gold = value;
}

/// Party reputation. Stored in the engine-specific GAM data as
/// `reputation * 10`; the existing accessor on `GamEngineData` does
/// the divide. We mirror that contract: read returns the
/// player-facing value (0..=20-ish), write multiplies by 10 on the
/// way to disk.
pub fn party_reputation(gam: &ImportedGam) -> u32 {
    gam.engine_data.reputation()
}

pub fn set_party_reputation(gam: &mut ImportedGam, value: u32) {
    let raw = value.saturating_mul(10);
    match &mut gam.engine_data {
        GamEngineData::Bg(d) => set_bg_reputation(d, raw),
        GamEngineData::Bg2(d) => set_bg2_reputation(d, raw),
        GamEngineData::Ee(d) => set_ee_reputation(d, raw),
        GamEngineData::Iwd(d) => set_iwd_reputation(d, raw),
        GamEngineData::Iwd2(d) => set_iwd2_reputation(d, raw),
        GamEngineData::Pst(d) => set_pst_reputation(d, raw),
    }
}

// Each engine variant exposes `reputation` as a public field — the
// trivial assignments below stay one-liners so the dispatch above
// reads like a flat table.
fn set_bg_reputation(d: &mut BgGamData, v: u32) {
    d.reputation = v;
}
fn set_bg2_reputation(d: &mut Bg2GamData, v: u32) {
    d.reputation = v;
}
fn set_ee_reputation(d: &mut EeGamData, v: u32) {
    d.reputation = v;
}
fn set_iwd_reputation(d: &mut IwdGamData, v: u32) {
    d.reputation = v;
}
fn set_iwd2_reputation(d: &mut Iwd2GamData, v: u32) {
    d.reputation = v;
}
fn set_pst_reputation(d: &mut PstGamData, v: u32) {
    d.reputation = v;
}
