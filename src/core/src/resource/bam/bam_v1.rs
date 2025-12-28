use std::io::BufRead;

use crate::{datasource::Reader, resource::bam::Bam};

/// A BAM V1 file importer
pub struct BamV1Parser;

impl BamV1Parser {
    /// Imports a BAM V1 file
    pub fn import<R: BufRead>(reader: &mut Reader<R>) -> std::io::Result<Bam> {
        todo!()
    }
}