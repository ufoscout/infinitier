
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Reads a directory and returns a map of all the files in it
/// recursively and their absolute path lowercased
pub fn list_real_entries_recursive(path: &Path) -> io::Result<HashMap<String, PathBuf>> {
    let mut results = HashMap::new();
    recurse(path, &mut results)?;
    Ok(results)
}

fn recurse(path: &Path, results: &mut HashMap<String, PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let entry_path = entry.path();

        let abs_path = entry_path.canonicalize()?.to_str().expect(&format!("Cannot convert path to string {}", path.display())).to_lowercase();

        if metadata.is_file() {
            results.insert(abs_path,entry_path);
        } else if metadata.is_dir() {
            recurse(&entry_path, results)?;
            results.insert(abs_path, entry_path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_real_entries_recursive() {
        let current_path = std::env::current_dir().unwrap();
        let results = list_real_entries_recursive(&current_path).unwrap();
        println!("Results: {:#?}", results);
        assert_eq!(results.len(), 1);
    }
}