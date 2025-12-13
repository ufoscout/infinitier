use std::collections::HashMap;

use itertools::{Itertools, chain};
use log::warn;

use crate::datasource::{DataSource, Importer};

/// A Wed file importer
pub struct WedImporter;

impl Importer for WedImporter {
    type T = Wed;

    fn import(source: &DataSource) -> std::io::Result<Wed> {
        let mut reader = source.reader()?;

        let signature = reader.read_string(8)?;

        if signature != "WED V1.3" {
            return Err(std::io::Error::other("Wrong file type"));
        }

        let overlays_size = reader.read_u32()?  as u64;
        let doors_size = reader.read_u32()? as u64;
        let overlays_offset = reader.read_u32()? as u64;
        let secondary_header_offset = reader.read_u32()? as u64;
        let doors_offset = reader.read_u32()? as u64;
        let door_tiles_offset = reader.read_u32()? as u64;

        reader.set_position(overlays_offset)?;

        // let default = reader.read_line()?.0.trim().to_string();
        // let (headers, columns) = parse_headers(&reader.read_line()?.0);

        let mut rows = HashMap::new();
        // loop {
        //     let (line, bytes) = reader.read_line()?;
        //     if bytes == 0 {
        //         break;
        //     }
        //     let (key, value) = parse_data_row(line.trim(), &columns, &default);
        //     rows.insert(key, value);
        // }

        Ok(Wed {
            headers: Vec::new(),
            columns: Vec::new(),
            rows,
        })
    }
}

/// Represents a 2DA file.
pub struct Wed {
    pub headers: Vec<String>,
    pub columns: Vec<usize>,
    pub rows: HashMap<String, Vec<String>>,
}



#[cfg(test)]
mod tests {
    use crate::{fs::{CaseInsensitiveFS, CaseInsensitivePath}, test_utils::BG2_RESOURCES_DIR};
    use super::*;

        #[test]
    fn test_parse_wed_file() {
        let path = CaseInsensitiveFS::new(BG2_RESOURCES_DIR)
            .unwrap()
            .get_path(&CaseInsensitivePath::new("override/ar0072.WED"))
            .unwrap();
        let wed = WedImporter::import(&DataSource::new(path)).unwrap();

    }

}