//! Minimal CRE (creature) reader — just enough to extract ability
//! scores from an embedded CRE blob. The MVP only needs the seven
//! ability scores; other CRE fields will follow as the editor grows.
//!
//! Spec references (IESDP):
//! - `cre_v1.htm`  — V1.0 (BG1, BG2, BG:EE / BG2:EE / IWD:EE / EET)
//! - `cre_v1.2.htm` — V1.2 (PST / PST:EE)
//! - `cre_v9.htm`  — V9.0 (IWD vanilla / HoW)
//! - `cre_v2.2.htm` — V2.2 (IWD2; d20 system, no Strength % bonus)
//!
//! V1.0 / V1.2 / V9.0 share the same ability-score layout (7 bytes
//! at 0x0238..0x023F):
//!
//! | Offset | Field            |
//! |--------|------------------|
//! | 0x0238 | Strength         |
//! | 0x0239 | Strength % bonus |
//! | 0x023A | Intelligence     |
//! | 0x023B | Wisdom           |
//! | 0x023C | Dexterity        |
//! | 0x023D | Constitution     |
//! | 0x023E | Charisma         |
//!
//! V2.2 (IWD2 / d20) drops the % bonus and shifts the block:
//!
//! | Offset | Field        |
//! |--------|--------------|
//! | 0x0266 | Strength     |
//! | 0x0267 | Intelligence |
//! | 0x0268 | Wisdom       |
//! | 0x0269 | Dexterity    |
//! | 0x026A | Constitution |
//! | 0x026B | Charisma     |

use std::io::Read;

use infinitier_datasource::{DataSource, ReadExt};

/// 4-byte signature at offset 0 of every CRE blob.
pub const CRE_SIGNATURE: &[u8; 4] = b"CRE ";

/// On-disk CRE version. We dispatch ability-score offsets on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreVersion {
    /// `V1.0` — BG1, BG2, BG:EE, BG2:EE, IWD:EE, EET.
    V1_0,
    /// `V1.2` — PST, PST:EE.
    V1_2,
    /// `V9.0` — IWD vanilla, HoW.
    V9_0,
    /// `V2.2` — IWD2 (d20 system, no Strength % bonus).
    V2_2,
}

impl CreVersion {
    /// The 4-byte tag stored at offset 0x04 of the CRE blob. Used by
    /// the (future) CRE writer to round-trip the version field.
    #[allow(dead_code)]
    pub fn as_bytes(&self) -> &'static [u8; 4] {
        match self {
            CreVersion::V1_0 => b"V1.0",
            CreVersion::V1_2 => b"V1.2",
            CreVersion::V9_0 => b"V9.0",
            CreVersion::V2_2 => b"V2.2",
        }
    }

    /// File offset of the Strength byte for this version.
    fn strength_offset(self) -> u64 {
        match self {
            CreVersion::V1_0 | CreVersion::V1_2 | CreVersion::V9_0 => 0x0238,
            CreVersion::V2_2 => 0x0266,
        }
    }

    /// Whether this version stores the AD&D-style `Strength % Bonus`
    /// byte immediately after Strength (used for 18/01..18/00).
    pub fn has_strength_bonus(self) -> bool {
        match self {
            CreVersion::V1_0 | CreVersion::V1_2 | CreVersion::V9_0 => true,
            CreVersion::V2_2 => false,
        }
    }
}

/// The seven (or six, for IWD2) ability-score values pulled from a
/// CRE blob.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abilities {
    pub strength: u8,
    /// `Some(0..=100)` for AD&D-era engines (the 18/01..18/00 bonus);
    /// `None` for IWD2's d20 system.
    pub strength_bonus: Option<u8>,
    pub intelligence: u8,
    pub wisdom: u8,
    pub dexterity: u8,
    pub constitution: u8,
    pub charisma: u8,
}

/// A minimally-parsed CRE blob. Currently exposes only the version
/// tag and the ability scores; the rest of the file is the caller's
/// problem (and grows as the editor learns more fields).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreSummary {
    pub version: CreVersion,
    pub abilities: Abilities,
}

/// Parse `bytes` as a CRE blob and return the summary. The blob is
/// the raw self-contained creature record — typically obtained via
/// [`infinitier_gam_resource::GamNpc::cre_data`] for an embedded
/// party member.
pub fn parse_cre(bytes: &[u8]) -> std::io::Result<CreSummary> {
    let mut reader = DataSource::new(bytes).preloaded_reader()?;
    let mut sig = [0u8; 4];
    reader.read_exact(&mut sig)?;
    if &sig != CRE_SIGNATURE {
        return Err(std::io::Error::other(format!(
            "Unsupported CRE signature: {sig:?}"
        )));
    }
    let mut ver = [0u8; 4];
    reader.read_exact(&mut ver)?;
    let version = match &ver {
        b"V1.0" => CreVersion::V1_0,
        b"V1.2" => CreVersion::V1_2,
        b"V9.0" => CreVersion::V9_0,
        b"V2.2" => CreVersion::V2_2,
        _ => {
            return Err(std::io::Error::other(format!(
                "Unsupported CRE version: {ver:?}"
            )));
        }
    };

    use infinitier_datasource::SeekExt;
    reader.set_position(version.strength_offset())?;
    let strength = reader.read_u8()?;
    let strength_bonus = if version.has_strength_bonus() {
        Some(reader.read_u8()?)
    } else {
        None
    };
    let intelligence = reader.read_u8()?;
    let wisdom = reader.read_u8()?;
    let dexterity = reader.read_u8()?;
    let constitution = reader.read_u8()?;
    let charisma = reader.read_u8()?;

    Ok(CreSummary {
        version,
        abilities: Abilities {
            strength,
            strength_bonus,
            intelligence,
            wisdom,
            dexterity,
            constitution,
            charisma,
        },
    })
}
