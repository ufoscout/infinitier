use std::{io, path::Path};

use encoding_rs::WINDOWS_1252;

use crate::io::Reader;

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    Bif,
    Biff,
    Bifc,
}

pub fn detect_biff_type(file_path: &Path) -> Result<Type, io::Error> {
    if !file_path.is_file() {
        return Err(io::Error::other("Not a file"));
    }

    let mut reader = Reader::with_file(file_path, WINDOWS_1252)?;
    let value = reader.read_string(8)?;

    match value.as_str() {
        "BIFFV1  " => Ok(Type::Biff),
        "BIF V1.0" => Ok(Type::Bif),
        "BIFCV1.0" => Ok(Type::Bifc),
        val => Err(io::Error::other(format!("Unsupported BIFF file: {}", val))),
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::RESOURCES_DIR;

    use super::*;

    #[test]
    fn test_detect_bif_type() {
        assert_eq!(
            detect_biff_type(Path::new(&format!(
                "{RESOURCES_DIR}iwd/CD2/Data/AR3603.cbf"
            )))
            .unwrap(),
            Type::Bif
        );
    }

    #[test]
    fn test_detect_biff_type() {
        assert_eq!(
            detect_biff_type(Path::new(&format!("{RESOURCES_DIR}pst/CS_0511.bif"))).unwrap(),
            Type::Biff
        );
    }
}
