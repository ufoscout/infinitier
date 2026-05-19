use std::io::{BufRead, Read, Seek};

use infinitier_datasource::{Importer, Reader, SeekExt};
use log::{debug, error};

use crate::{MOS_V1_SIGNATURE, MOS_V2_SIGNATURE, MOSC_V1_SIGNATURE, Mos, Type};

pub(crate) mod mosc;
pub(crate) mod v1;
pub(crate) mod v2;

use mosc::MoscParser;
use v1::MosV1Parser;
use v2::MosV2Parser;

/// A MOS file importer.
pub struct MosImporter<'a> {
    pub name: &'a str,
}

impl Importer for MosImporter<'_> {
    type T = Mos;

    fn import(&self, source: &infinitier_datasource::DataSource) -> std::io::Result<Self::T> {
        let reader = &mut source.reader()?;
        let mos = MosImporter::from_reader(reader)?;
        debug!("Loaded {} [MOS]", self.name);
        Ok(mos)
    }
}

impl MosImporter<'_> {
    /// Imports a MOS file (V1, V2, or MOSC) from an arbitrary reader.
    ///
    /// The dispatcher peeks at the 8-byte signature, rewinds, and delegates
    /// to the variant-specific parser — mirroring `BamImporter::from_reader`.
    pub fn from_reader<R: BufRead + Seek>(reader: &mut Reader<R>) -> std::io::Result<Mos> {
        let position = reader.position()?;

        match detect_mos_type(reader)? {
            Type::MosV1 => {
                reader.set_position(position)?;
                MosV1Parser::import(reader).map(Mos::V1)
            }
            Type::MosV2 => {
                reader.set_position(position)?;
                MosV2Parser::import(reader).map(Mos::V2)
            }
            Type::Mosc => {
                reader.set_position(position)?;
                MoscParser::import(reader)
            }
        }
    }
}

/// Detects the variant of a MOS file from its first 8 bytes.
///
/// `MOS V1  `, `MOS V2  `, and `MOSCV1  ` are the only valid signatures.
/// The reader is left positioned just after the 8th byte.
pub fn detect_mos_type<R: Read>(reader: &mut Reader<R>) -> std::io::Result<Type> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    match &buf {
        b if b == MOS_V1_SIGNATURE => Ok(Type::MosV1),
        b if b == MOS_V2_SIGNATURE => Ok(Type::MosV2),
        b if b == MOSC_V1_SIGNATURE => Ok(Type::Mosc),
        _ => {
            error!("Unsupported MOS file signature: {:?}", buf);
            Err(std::io::Error::other(format!(
                "Unsupported MOS file signature: {:?}",
                buf
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
    fn test_detect_mos_v1_type() {
        let data = DataSource::new(get_assets_path().join("MOS/V1/GTRSPCAP.mos"));
        assert_eq!(
            detect_mos_type(&mut data.reader().unwrap()).unwrap(),
            Type::MosV1
        );
    }

    #[test]
    fn test_detect_mos_v2_type() {
        let data = DataSource::new(get_assets_path().join("MOS/V2/BGDECBAR.mos"));
        assert_eq!(
            detect_mos_type(&mut data.reader().unwrap()).unwrap(),
            Type::MosV2
        );
    }

    #[test]
    fn test_detect_mosc_type() {
        let data = DataSource::new(get_assets_path().join("MOS/MOSC/GUIWRLP8.mos"));
        assert_eq!(
            detect_mos_type(&mut data.reader().unwrap()).unwrap(),
            Type::Mosc
        );
    }

    #[test]
    fn test_detect_rejects_garbage() {
        let data = DataSource::new(&b"GARBAGE!"[..]);
        let err = detect_mos_type(&mut data.reader().unwrap()).unwrap_err();
        assert!(err.to_string().contains("Unsupported MOS file signature"));
    }
}
