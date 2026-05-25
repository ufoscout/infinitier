use std::path::{Path, PathBuf};

pub trait Roots {
    fn pathbufs(&self) -> Vec<PathBuf>;
}

impl Roots for Path {
    fn pathbufs(&self) -> Vec<PathBuf> {
        vec![self.to_path_buf()]
    }
}

impl Roots for PathBuf {
    fn pathbufs(&self) -> Vec<PathBuf> {
        vec![self.clone()]
    }
}

impl Roots for str {
    fn pathbufs(&self) -> Vec<PathBuf> {
        vec![PathBuf::from(self)]
    }
}

impl Roots for String {
    fn pathbufs(&self) -> Vec<PathBuf> {
        vec![PathBuf::from(self)]
    }
}

impl<T: AsRef<Path>> Roots for [T] {
    fn pathbufs(&self) -> Vec<PathBuf> {
        self.iter().map(|p| p.as_ref().to_path_buf()).collect()
    }
}

impl<T: AsRef<Path>> Roots for Vec<T> {
    fn pathbufs(&self) -> Vec<PathBuf> {
        self.iter().map(|p| p.as_ref().to_path_buf()).collect()
    }
}

impl<T: Roots + ?Sized> Roots for &T {
    fn pathbufs(&self) -> Vec<PathBuf> {
        (**self).pathbufs()
    }
}
