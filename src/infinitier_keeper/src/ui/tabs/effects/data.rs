//! Read-only extraction of a creature's effects for the Effects tab.
//!
//! EEKeeper's Effects tab lists the EE (V2) effect records on the
//! creature, **excluding** the two opcodes that have their own
//! dedicated tabs — `op233` (set proficiency → Proficiencies tab) and
//! `op187` (set local variable → Local Variables tab). Those parse to
//! the [`EffectV2::Proficiency`] / [`EffectV2::LocalVariable`] variants,
//! so iterating only the fully-parsed [`Effect`](infinitier_core::resource::cre::Effect)
//! variant naturally skips them.
//!
//! The CRE importer already parses every field, so this module just
//! projects the ones the tab shows; name resolution (opcode →
//! effect-name, resref → spell-name, timing/target → text) happens in
//! the view, which has `GameData`.

use infinitier_core::resource::cre::{Cre, EffectList, EffectV2, SubSections};

/// One Effects-tab row, before name / text resolution.
pub struct EffectRow {
    pub opcode: u32,
    pub name: String,
    pub param1: u32,
    pub param2: u32,
    /// Resource 0..3 resrefs (empty string when unset): the effect's
    /// resource, resource2, resource3 and parent-resource fields.
    pub resources: [String; 4],
    pub time: u32,
    pub timing_mode: u32,
    pub target: u32,
}

/// Build the effect rows for a creature. Only EE (V2) effects are
/// shown; classic 48-byte (V1) effect lists aren't modelled as typed
/// effects and yield an empty table here.
pub fn effect_rows(cre: &Cre) -> Vec<EffectRow> {
    let SubSections::V1(sub) = &cre.sub_sections else {
        return Vec::new();
    };
    let EffectList::V2(effects) = &sub.effects else {
        return Vec::new();
    };

    effects
        .iter()
        .filter_map(|e| {
            // Only the generic `Effect` variant is shown here; the
            // proficiency / local-variable variants have their own tabs.
            let EffectV2::Effect(e) = e else { return None };
            Some(EffectRow {
                opcode: e.opcode,
                name: e.variable.clone(),
                param1: e.param1,
                param2: e.param2,
                resources: [
                    e.resource.clone(),
                    e.resource2.clone(),
                    e.resource3.clone(),
                    e.parent_resource.clone(),
                ],
                time: e.duration,
                timing_mode: e.timing_mode,
                target: e.target,
            })
        })
        .collect()
}
