
use std::{path::PathBuf, sync::Arc};

use encoding_rs::{Encoding, WINDOWS_1252};
use flate2::bufread::ZlibDecoder;

use crate::TempFileGenerator;


/// A data source
#[derive(Debug, Clone)]
pub enum DataSource {
    Path(PathBuf),
    Generator(Arc<TempFileGenerator>),
    MemorySource(Arc<Vec<u8>>),
}