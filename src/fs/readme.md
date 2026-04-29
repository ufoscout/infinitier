# infinitier_fs

A case-insensitive filesystem abstraction for case-sensitive operating systems.

`CaseInsensitiveFS` indexes a directory tree at construction time, mapping every entry to its lowercased relative path. All subsequent lookups are O(log n) in-memory operations with no further filesystem access. Path normalisation handles mixed separators (`\` and `/`) and leading slashes automatically.

`search_path_opt` extends plain lookups by also checking a set of predefined subdirectories (`data/`, `cache/`, `cd1/`–`cd7/`), useful when resources may be spread across several subdirectories without knowing in advance which one they live in.

## Usage

### Look up a file by case-insensitive path

```rust,no_run
use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fs = CaseInsensitiveFS::new("/some/root")?;

    // Resolves regardless of actual on-disk casing
    let path = fs.get_path(&CaseInsensitivePath::new("Config.ini"))?;
    println!("{}", path.display());
    Ok(())
}
```

### Search across subdirectories

`search_path_opt` checks the root first, then `data/`, `cache/`, `cd1/`–`cd7/` automatically.

```rust,no_run
use infinitier_fs::{CaseInsensitiveFS, CaseInsensitivePath};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let fs = CaseInsensitiveFS::new("/some/root")?;

    let path = fs.search_path_opt(&CaseInsensitivePath::new("resources/archive.bin"));
    println!("{:?}", path);
    Ok(())
}
```
