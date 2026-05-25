use serde::{Deserialize, Serialize};

/// A Resource file type
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SaveGameResourceType {
    Sav,
}

impl SaveGameResourceType {
    pub fn from_extension(ext: &str) -> Option<SaveGameResourceType> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        match ext.as_str() {
            "sav" => Some(SaveGameResourceType::Sav),
            _ => None,
        }
    }

    /// Returns the extension of the `ResourceType` enum variant as a string, or `None` if it is unknown.
    pub fn get_extension(&self) -> &'static str {
        match self {
            SaveGameResourceType::Sav => "sav",
        }
    }
}

/// A Resource file type
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    Acm,
    Are,
    Bah,
    Bam,
    Bcs,
    Bio,
    Bmp,
    Bs,
    Chr,
    Chu,
    Cre,
    Dlg,
    Eff,
    Fnt,
    Gam,
    Glsl,
    Gui,
    Ids,
    Ini,
    Itm,
    Lua,
    Maze,
    Menu,
    Mos,
    Mus,
    Mve,
    Plt,
    Png,
    Pro,
    Pvrz,
    Spl,
    Sql,
    Src,
    Sto,
    Tga,
    Tis,
    Toh,
    Tot,
    Ttf,
    TwoDA,
    Vef,
    Vvc,
    Wav,
    Wbm,
    Wed,
    Wfx,
    Wmp,
    Unknown(u16),
}

impl ResourceType {
    /// Returns the `ResourceType` enum variant based on the given hexadecimal value.
    pub fn from(bit: u16) -> Self {
        match bit {
            0x001 => ResourceType::Bmp,
            0x002 => ResourceType::Mve,
            0x004 => ResourceType::Wav,
            0x005 => ResourceType::Wfx,
            0x006 => ResourceType::Plt,
            0x3b8 => ResourceType::Tga,
            0x3e8 => ResourceType::Bam,
            0x3e9 => ResourceType::Wed,
            0x3ea => ResourceType::Chu,
            0x3eb => ResourceType::Tis,
            0x3ec => ResourceType::Mos,
            0x3ed => ResourceType::Itm,
            0x3ee => ResourceType::Spl,
            0x3ef => ResourceType::Bcs,
            0x3f0 => ResourceType::Ids,
            0x3f1 => ResourceType::Cre,
            0x3f2 => ResourceType::Are,
            0x3f3 => ResourceType::Dlg,
            0x3f4 => ResourceType::TwoDA,
            0x3f5 => ResourceType::Gam,
            0x3f6 => ResourceType::Sto,
            0x3f7 => ResourceType::Wmp,
            0x3f8 => ResourceType::Eff,
            0x3f9 => ResourceType::Bs,
            0x3fa => ResourceType::Chr,
            0x3fb => ResourceType::Vvc,
            0x3fc => ResourceType::Vef,
            0x3fd => ResourceType::Pro,
            0x3fe => ResourceType::Bio,
            0x3ff => ResourceType::Wbm,
            0x400 => ResourceType::Fnt,
            0x402 => ResourceType::Gui,
            0x403 => ResourceType::Sql,
            0x404 => ResourceType::Pvrz,
            0x405 => ResourceType::Glsl,
            0x406 => ResourceType::Tot,
            0x407 => ResourceType::Toh,
            0x408 => ResourceType::Menu,
            0x409 => ResourceType::Lua,
            0x40a => ResourceType::Ttf,
            0x40b => ResourceType::Png,
            0x44c => ResourceType::Bah,
            0x802 => ResourceType::Ini,
            0x803 => ResourceType::Src,
            0x804 => ResourceType::Maze,
            0xffe => ResourceType::Mus,
            0xfff => ResourceType::Acm,
            i => ResourceType::Unknown(i),
        }
    }

    /// Returns the hexadecimal value of the `ResourceType` enum variant.
    pub fn to_u16(&self) -> u16 {
        match self {
            ResourceType::Bmp => 0x001,
            ResourceType::Mve => 0x002,
            ResourceType::Wav => 0x004,
            ResourceType::Wfx => 0x005,
            ResourceType::Plt => 0x006,
            ResourceType::Tga => 0x3b8,
            ResourceType::Bam => 0x3e8,
            ResourceType::Wed => 0x3e9,
            ResourceType::Chu => 0x3ea,
            ResourceType::Tis => 0x3eb,
            ResourceType::Mos => 0x3ec,
            ResourceType::Itm => 0x3ed,
            ResourceType::Spl => 0x3ee,
            ResourceType::Bcs => 0x3ef,
            ResourceType::Ids => 0x3f0,
            ResourceType::Cre => 0x3f1,
            ResourceType::Are => 0x3f2,
            ResourceType::Dlg => 0x3f3,
            ResourceType::TwoDA => 0x3f4,
            ResourceType::Gam => 0x3f5,
            ResourceType::Sto => 0x3f6,
            ResourceType::Wmp => 0x3f7,
            ResourceType::Eff => 0x3f8,
            ResourceType::Bs => 0x3f9,
            ResourceType::Chr => 0x3fa,
            ResourceType::Vvc => 0x3fb,
            ResourceType::Vef => 0x3fc,
            ResourceType::Pro => 0x3fd,
            ResourceType::Bio => 0x3fe,
            ResourceType::Wbm => 0x3ff,
            ResourceType::Fnt => 0x400,
            ResourceType::Gui => 0x402,
            ResourceType::Sql => 0x403,
            ResourceType::Pvrz => 0x404,
            ResourceType::Glsl => 0x405,
            ResourceType::Tot => 0x406,
            ResourceType::Toh => 0x407,
            ResourceType::Menu => 0x408,
            ResourceType::Lua => 0x409,
            ResourceType::Ttf => 0x40a,
            ResourceType::Png => 0x40b,
            ResourceType::Bah => 0x44c,
            ResourceType::Ini => 0x802,
            ResourceType::Src => 0x803,
            ResourceType::Maze => 0x804,
            ResourceType::Mus => 0xffe,
            ResourceType::Acm => 0xfff,
            ResourceType::Unknown(i) => *i,
        }
    }

    /// Returns the `ResourceType` matching the given file extension,
    /// or `None` if no known type matches. Matching is case-insensitive
    /// and ignores a leading `.`.
    pub fn from_extension(ext: &str) -> Option<ResourceType> {
        let ext = ext.trim_start_matches('.').to_ascii_lowercase();
        match ext.as_str() {
            "bmp" => Some(ResourceType::Bmp),
            "mve" => Some(ResourceType::Mve),
            "wav" => Some(ResourceType::Wav),
            "wfx" => Some(ResourceType::Wfx),
            "plt" => Some(ResourceType::Plt),
            "tga" => Some(ResourceType::Tga),
            "bam" => Some(ResourceType::Bam),
            "wed" => Some(ResourceType::Wed),
            "chu" => Some(ResourceType::Chu),
            "tis" => Some(ResourceType::Tis),
            "mos" => Some(ResourceType::Mos),
            "itm" => Some(ResourceType::Itm),
            "spl" => Some(ResourceType::Spl),
            "bcs" => Some(ResourceType::Bcs),
            "ids" => Some(ResourceType::Ids),
            "cre" => Some(ResourceType::Cre),
            "are" => Some(ResourceType::Are),
            "dlg" => Some(ResourceType::Dlg),
            "2da" => Some(ResourceType::TwoDA),
            "gam" => Some(ResourceType::Gam),
            "sto" => Some(ResourceType::Sto),
            "wmp" => Some(ResourceType::Wmp),
            "eff" => Some(ResourceType::Eff),
            "bs" => Some(ResourceType::Bs),
            "chr" => Some(ResourceType::Chr),
            "vvc" => Some(ResourceType::Vvc),
            "vef" => Some(ResourceType::Vef),
            "pro" => Some(ResourceType::Pro),
            "bio" => Some(ResourceType::Bio),
            "wbm" => Some(ResourceType::Wbm),
            "fnt" => Some(ResourceType::Fnt),
            "gui" => Some(ResourceType::Gui),
            "sql" => Some(ResourceType::Sql),
            "pvrz" => Some(ResourceType::Pvrz),
            "glsl" => Some(ResourceType::Glsl),
            "tot" => Some(ResourceType::Tot),
            "toh" => Some(ResourceType::Toh),
            "menu" => Some(ResourceType::Menu),
            "lua" => Some(ResourceType::Lua),
            "ttf" => Some(ResourceType::Ttf),
            "png" => Some(ResourceType::Png),
            "bah" => Some(ResourceType::Bah),
            "ini" => Some(ResourceType::Ini),
            "src" => Some(ResourceType::Src),
            "maze" => Some(ResourceType::Maze),
            "mus" => Some(ResourceType::Mus),
            "acm" => Some(ResourceType::Acm),
            _ => None,
        }
    }

    /// Returns the extension of the `ResourceType` enum variant as a string, or `None` if it is unknown.
    pub fn get_extension(&self) -> Option<&'static str> {
        match self {
            ResourceType::Bmp => Some("bmp"),
            ResourceType::Mve => Some("mve"),
            ResourceType::Wav => Some("wav"),
            ResourceType::Wfx => Some("wfx"),
            ResourceType::Plt => Some("plt"),
            ResourceType::Tga => Some("tga"),
            ResourceType::Bam => Some("bam"),
            ResourceType::Wed => Some("wed"),
            ResourceType::Chu => Some("chu"),
            ResourceType::Tis => Some("tis"),
            ResourceType::Mos => Some("mos"),
            ResourceType::Itm => Some("itm"),
            ResourceType::Spl => Some("spl"),
            ResourceType::Bcs => Some("bcs"),
            ResourceType::Ids => Some("ids"),
            ResourceType::Cre => Some("cre"),
            ResourceType::Are => Some("are"),
            ResourceType::Dlg => Some("dlg"),
            ResourceType::TwoDA => Some("2da"),
            ResourceType::Gam => Some("gam"),
            ResourceType::Sto => Some("sto"),
            ResourceType::Wmp => Some("wmp"),
            ResourceType::Eff => Some("eff"),
            ResourceType::Bs => Some("bs"),
            ResourceType::Chr => Some("chr"),
            ResourceType::Vvc => Some("vvc"),
            ResourceType::Vef => Some("vef"),
            ResourceType::Pro => Some("pro"),
            ResourceType::Bio => Some("bio"),
            ResourceType::Wbm => Some("wbm"),
            ResourceType::Fnt => Some("fnt"),
            ResourceType::Gui => Some("gui"),
            ResourceType::Sql => Some("sql"),
            ResourceType::Pvrz => Some("pvrz"),
            ResourceType::Glsl => Some("glsl"),
            ResourceType::Tot => Some("tot"),
            ResourceType::Toh => Some("toh"),
            ResourceType::Menu => Some("menu"),
            ResourceType::Lua => Some("lua"),
            ResourceType::Ttf => Some("ttf"),
            ResourceType::Png => Some("png"),
            ResourceType::Bah => Some("bah"),
            ResourceType::Ini => Some("ini"),
            ResourceType::Src => Some("src"),
            ResourceType::Maze => Some("maze"),
            ResourceType::Mus => Some("mus"),
            ResourceType::Acm => Some("acm"),
            ResourceType::Unknown(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_type_roundtrip() {
        for i in 0..16u16.pow(3) {
            assert_eq!(ResourceType::from(i).to_u16(), i);
        }
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(ResourceType::TwoDA.get_extension(), Some("2da"));
        assert_eq!(ResourceType::Unknown(0).get_extension(), None);
        assert_eq!(ResourceType::Bmp.get_extension(), Some("bmp"));
    }

    #[test]
    fn test_from_extension_known() {
        assert_eq!(ResourceType::from_extension("bmp"), Some(ResourceType::Bmp));
        assert_eq!(
            ResourceType::from_extension("2da"),
            Some(ResourceType::TwoDA)
        );
        assert_eq!(ResourceType::from_extension("acm"), Some(ResourceType::Acm));
        assert_eq!(
            ResourceType::from_extension("pvrz"),
            Some(ResourceType::Pvrz)
        );
    }

    #[test]
    fn test_from_extension_case_insensitive() {
        assert_eq!(ResourceType::from_extension("BMP"), Some(ResourceType::Bmp));
        assert_eq!(ResourceType::from_extension("Wav"), Some(ResourceType::Wav));
        assert_eq!(
            ResourceType::from_extension("2DA"),
            Some(ResourceType::TwoDA)
        );
        assert_eq!(
            ResourceType::from_extension("MeNu"),
            Some(ResourceType::Menu)
        );
    }

    #[test]
    fn test_from_extension_strips_leading_dot() {
        assert_eq!(
            ResourceType::from_extension(".bmp"),
            Some(ResourceType::Bmp)
        );
        assert_eq!(
            ResourceType::from_extension(".2DA"),
            Some(ResourceType::TwoDA)
        );
    }

    #[test]
    fn test_from_extension_unknown() {
        assert_eq!(ResourceType::from_extension(""), None);
        assert_eq!(ResourceType::from_extension("xyz"), None);
        assert_eq!(ResourceType::from_extension("unknown"), None);
        // No partial / fuzzy matches
        assert_eq!(ResourceType::from_extension("bm"), None);
        assert_eq!(ResourceType::from_extension("bmpx"), None);
    }

    #[test]
    fn test_from_extension_roundtrip_via_get_extension() {
        for bit in 0..16u16.pow(3) {
            let r#type = ResourceType::from(bit);
            if let Some(ext) = r#type.get_extension() {
                assert_eq!(
                    ResourceType::from_extension(ext),
                    Some(r#type),
                    "extension {ext} did not round-trip for {:?}",
                    r#type
                );
            }
        }
    }
}
