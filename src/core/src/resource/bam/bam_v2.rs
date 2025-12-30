use std::io::BufRead;

use crate::{datasource::Reader, resource::bam::Bam};

/// A BAM V2 file importer
pub struct BamV2Parser;

impl BamV2Parser {
    /// Imports a BAM V2 file
    pub fn import<R: BufRead>(_reader: &mut Reader<R>) -> std::io::Result<Bam> {
        todo!()
    }
}
