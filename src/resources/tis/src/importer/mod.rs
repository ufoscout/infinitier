use std::io::{BufRead, Read, Seek};

use infinitier_datasource::{Importer, ReadExt, Reader, SeekExt};
use log::{debug, error};

use crate::{PALETTE_TILE_LENGTH, PVRZ_TILE_LENGTH, TIS_SIGNATURE, Tis, Type};

pub(crate) mod palette;
pub(crate) mod pvrz;

use palette::TisPaletteParser;
use pvrz::TisPvrzParser;

/// A TIS file importer.
pub struct TisImporter<'a> {
    pub name: &'a str,
}

impl Importer for TisImporter<'_> {
    type T = Tis;

    fn import(&self, source: &infinitier_datasource::DataSource) -> std::io::Result<Self::T> {
        let reader = &mut source.reader()?;
        let tis = TisImporter::from_reader(reader)?;
        debug!("Loaded {} [TIS]", self.name);
        Ok(tis)
    }
}

impl TisImporter<'_> {
    /// Imports a TIS file from an arbitrary reader.
    ///
    /// The dispatcher reads the 24-byte header, picks the variant from
    /// the `tile_length` field, rewinds, and delegates to the
    /// variant-specific parser (same shape as
    /// [`MosImporter::from_reader`](infinitier_datasource)).
    pub fn from_reader<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<Tis> {
        let position = reader.position()?;

        match detect_tis_type(reader)? {
            Type::Palette => {
                reader.set_position(position)?;
                TisPaletteParser::import(reader).map(Tis::Palette)
            }
            Type::Pvrz => {
                reader.set_position(position)?;
                TisPvrzParser::import(reader).map(Tis::Pvrz)
            }
        }
    }
}

/// Detects the variant of a TIS file from its header.
///
/// The 8-byte signature is always `TIS V1  ` regardless of variant; the
/// variant is encoded in the `tile_length` field at offset `0x0C`:
/// `5120` → [`Type::Palette`], `12` → [`Type::Pvrz`]. The reader is left
/// positioned just after `tile_length` (offset `0x10`).
pub fn detect_tis_type<R: Read>(reader: &mut Reader<R>) -> std::io::Result<Type> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    if &buf != TIS_SIGNATURE {
        error!("Unsupported TIS file signature: {:?}", buf);
        return Err(std::io::Error::other(format!(
            "Unsupported TIS file signature: {:?}",
            buf
        )));
    }
    // Skip `tile_count` (u32) and read `tile_length` (u32).
    let _tile_count = reader.read_u32()?;
    let tile_length = reader.read_u32()?;
    match tile_length {
        PALETTE_TILE_LENGTH => Ok(Type::Palette),
        PVRZ_TILE_LENGTH => Ok(Type::Pvrz),
        n => {
            error!("Unsupported TIS tile_length: {}", n);
            Err(std::io::Error::other(format!(
                "Unsupported TIS tile_length: {} (expected {} for palette or {} for PVRZ)",
                n, PALETTE_TILE_LENGTH, PVRZ_TILE_LENGTH
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use infinitier_datasource::DataSource;
    use infinitier_test_utils::get_assets_path;

    use super::*;

    #[test]
    fn test_detect_palette_type() {
        let data = DataSource::new(get_assets_path().join("TIS/Palette/FIRE01.tis"));
        assert_eq!(
            detect_tis_type(&mut data.reader().unwrap()).unwrap(),
            Type::Palette
        );
    }

    #[test]
    fn test_detect_pvrz_type() {
        let data = DataSource::new(get_assets_path().join("TIS/Pvrz/AR0107.tis"));
        assert_eq!(
            detect_tis_type(&mut data.reader().unwrap()).unwrap(),
            Type::Pvrz
        );
    }

    #[test]
    fn test_detect_rejects_garbage_signature() {
        let data = DataSource::new(&b"GARBAGE!aaaaaaaaaaaaa"[..]);
        let err = detect_tis_type(&mut data.reader().unwrap()).unwrap_err();
        assert!(err.to_string().contains("Unsupported TIS file signature"));
    }

    #[test]
    fn test_detect_rejects_unknown_tile_length() {
        // Valid signature + dummy tile_count + bogus tile_length = 999.
        let mut bytes = b"TIS V1  ".to_vec();
        bytes.extend_from_slice(&1u32.to_le_bytes()); // tile_count
        bytes.extend_from_slice(&999u32.to_le_bytes()); // tile_length
        let data = DataSource::new(bytes);
        let err = detect_tis_type(&mut data.reader().unwrap()).unwrap_err();
        assert!(err.to_string().contains("Unsupported TIS tile_length"));
    }

    #[test]
    fn test_importer_dispatches_palette() {
        let data = DataSource::new(get_assets_path().join("TIS/Palette/FIRE01.tis"));
        let tis = TisImporter { name: "test" }.import(&data).unwrap();
        match tis {
            Tis::Palette(p) => {
                assert_eq!(p.r#type, Type::Palette);
                assert_eq!(p.tiles.len(), 1);
            }
            Tis::Pvrz(_) => panic!("expected Tis::Palette"),
        }
    }

    #[test]
    fn test_importer_dispatches_pvrz() {
        let data = DataSource::new(get_assets_path().join("TIS/Pvrz/AR0107.tis"));
        let tis = TisImporter { name: "test" }.import(&data).unwrap();
        match tis {
            Tis::Pvrz(p) => {
                assert_eq!(p.r#type, Type::Pvrz);
                assert_eq!(p.tiles.len(), 48);
            }
            Tis::Palette(_) => panic!("expected Tis::Pvrz"),
        }
    }
}
