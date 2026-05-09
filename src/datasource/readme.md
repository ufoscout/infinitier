# infinitier_datasource

An abstract data source that wraps a file path or an in-memory buffer behind a uniform `Reader` API with typed little-endian reads (`u8`/`u16`/`u32`/`u64`, `i8`/`i16`/`i32`), string decoding, seeking, and zlib decompression.

A `DataSource` can point at a whole file, a raw byte slice, or a sub-region of a file defined by an offset and an optional length limit, making it straightforward to read resources embedded inside archive formats. The default string encoding is Windows-1252 and can be overridden per source.

## Usage

### Read from a file

```rust,no_run
use infinitier_datasource::{DataSource, ReadExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new("data.bin");
    let mut reader = source.reader()?;
    let signature = reader.read_exact_to_array::<4>()?;
    let version   = reader.read_exact_to_array::<4>()?;
    let count     = reader.read_u32()?;
    println!("signature: {:?}, count: {}", signature, count);
    Ok(())
}
```

### Read from in-memory bytes

```rust
use infinitier_datasource::{DataSource, ReadExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = DataSource::new(b"\x01\x00\x02\x00" as &[u8]);
    let mut reader = source.reader()?;
    assert_eq!(reader.read_u16()?, 1);
    assert_eq!(reader.read_u16()?, 2);
    Ok(())
}
```

### Read a sub-region (embedded resource inside an archive)

```rust,no_run
use infinitier_datasource::{DataSource, ReadExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let offset: u64 = 1024;
    let size: u64   = 512;
    let source = DataSource::new_with_offset("archive.bin", offset, Some(size));
    let mut reader = source.reader()?;
    // reader is limited to the [offset, offset+size) byte range
    let value = reader.read_u32()?;
    println!("{}", value);
    Ok(())
}
```
