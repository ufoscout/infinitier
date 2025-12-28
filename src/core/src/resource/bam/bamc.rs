use std::io::BufRead;

use crate::{datasource::Reader, resource::bam::Bam};

/// A BAMC file importer
pub struct BamcParser;

impl BamcParser {
    /// Imports a BAMC file
    pub fn import<R: BufRead>(reader: &mut Reader<R>) -> std::io::Result<Bam> {
        todo!()
    }
}