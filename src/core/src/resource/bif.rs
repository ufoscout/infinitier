use std::{io, path::Path};

use encoding_rs::WINDOWS_1252;

use crate::io::Reader;

#[derive(Debug, PartialEq, Eq)]
enum Type {
    BIF,
    BIFF,
    BIFC,
}

fn detect_biff_type(file_path: &Path) -> Result<Type, io::Error> {
    if !file_path.is_file() {
        return Err(io::Error::other("Not a file"));
    }

    let mut reader = Reader::with_file(file_path, WINDOWS_1252)?;
    let value = reader.read_string(8)?;

    match value.as_str() {
        "BIFFV1  " => Ok(Type::BIFF),
        "BIF V1.0" => Ok(Type::BIF),
        "BIFCV1.0" => Ok(Type::BIFC),
        val => Err(io::Error::other(
            format!("Unsupported BIFF file: {}", val),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_biff_type() {
        assert_eq!(
            detect_biff_type(Path::new(
                "/home/ufo/Temp/Games/Baldur's Gate2 - Enhanced Edition/data/25effect.bif"
            ))
            .unwrap(),
            Type::BIFF
        );
    }
}
