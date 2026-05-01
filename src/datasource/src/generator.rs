use std::path::PathBuf;

use parking_lot::Mutex;

/// A temporary file generator
pub struct TempFileGenerator {
    generator: Box<dyn Fn() -> std::io::Result<PathBuf> + Send + Sync>,
    path: Mutex<PathBuf>,
}

impl std::fmt::Debug for TempFileGenerator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TempFileGenerator")
            .field("path", &*self.path.lock())
            .finish_non_exhaustive()
    }
}

impl TempFileGenerator {
    /// Creates a new temporary file generator
    pub fn new(generator: Box<dyn Fn() -> std::io::Result<PathBuf> + Send + Sync>) -> Self {
        Self {
            generator,
            path: Mutex::new(PathBuf::new()),
        }
    }

    /// Check if the file exists and return a path to it.
    /// If the file does not exist, it will be created and the path will be returned.
    pub fn path(&self) -> std::io::Result<PathBuf> {
        let mut path = self.path.lock();
        if !path.exists() {
            let tmpfile = (self.generator)()?;
            *path = tmpfile;
        }
        Ok(path.clone())
    }
}
