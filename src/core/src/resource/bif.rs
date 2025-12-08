
use crate::datasource::Reader;

#[derive(Debug, PartialEq, Eq)]
pub enum Type {
    Bif,
    Biff,
    Bifc,
}

/// Detects the type of a BIFF file
pub fn detect_biff_type(reader: &mut Reader) -> std::io::Result<Type> {

    let value = reader.read_string(8)?;

    match value.as_str() {
        "BIFFV1  " => Ok(Type::Biff),
        "BIF V1.0" => Ok(Type::Bif),
        "BIFCV1.0" => Ok(Type::Bifc),
        val => Err(std::io::Error::other(format!("Unsupported BIFF file: {}", val))),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{datasource::DataSource, test_utils::RESOURCES_DIR};

    use super::*;

    #[test]
    fn test_detect_bif_type() {
        let data = DataSource::new(Path::new(&format!(
                "{RESOURCES_DIR}iwd/CD2/Data/AR3603.cbf"
            )));

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap())
            .unwrap(),
            Type::Bif
        );
    }

    #[test]
    fn test_detect_biff_type() {
                let data = DataSource::new(Path::new(&format!(
                "{RESOURCES_DIR}pst/CS_0511.bif"
            )));

        assert_eq!(
            detect_biff_type(&mut data.reader().unwrap()).unwrap(),
            Type::Biff
        );
    }
}
