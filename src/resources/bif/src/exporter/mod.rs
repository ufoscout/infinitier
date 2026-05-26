use std::io::{self, BufWriter, Read, Write};
use std::path::Path;

use infinitier_datasource::SeekExt;

use crate::{BIFFV1_SIGNATURE, Bif, BifEmbeddedResource, Type};

mod bif;
mod bifc;

/// A BIF / BIFF / BIFC file exporter.
///
/// Writes a [`Bif`] back to one of the three Infinity Engine archive
/// formats. The choice between BIFF V1 (uncompressed), BIF V1.0 (single
/// zlib stream), and BIFC V1.0 (chunked zlib) is driven by `Bif::type`.
///
/// The output is functionally — not byte-exactly — equivalent to a source
/// archive: per-resource `offset` values are reassigned to a contiguous
/// packing scheme (no gaps between resources), and BIFC chunk boundaries
/// follow our own 32 KiB grid rather than the source's. Re-importing the
/// emitted bytes yields a [`Bif`] whose `name`, `type`, resource metadata
/// (locator, size, count, type) and per-resource byte payloads match the
/// original.
pub struct BifExporter;

/// Chunk size used when emitting BIFC V1.0 archives. Any value works since
/// the importer handles arbitrary chunk sizes; 32 KiB is a reasonable
/// compromise between compression effectiveness and per-chunk overhead.
pub(crate) const BIFC_CHUNK_SIZE: u64 = 32 * 1024;

impl BifExporter {
    /// Writes `bif` to `writer` in the format indicated by `bif.r#type`.
    pub fn export<W: Write>(&self, bif: &Bif, writer: &mut W) -> io::Result<()> {
        let biff_bytes = build_biff_v1(bif)?;
        match bif.r#type {
            Type::Biff => writer.write_all(&biff_bytes),
            Type::Bif => bif::write_bif_v1(&biff_bytes, &bif.name, writer),
            Type::Bifc => bifc::write_bifc_v1_0(&biff_bytes, writer),
        }
    }

    /// Writes `bif` to a file at `path`, creating or truncating it.
    pub fn export_to_file<P: AsRef<Path>>(&self, bif: &Bif, path: P) -> io::Result<()> {
        let file = std::fs::File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.export(bif, &mut writer)?;
        writer.flush()
    }
}

/// Builds an uncompressed BIFF V1 archive from `bif`. The output layout is:
///
/// ```text
/// offset 0    "BIFFV1  " (8 bytes)
/// offset 8    file_count        u32
/// offset 12   tileset_count     u32
/// offset 16   files_offset      u32   (= 16 + sum of resource payload sizes)
/// offset 20   <resource bytes, packed back-to-back, files-first then tilesets>
/// offset N    file entries  (16 bytes each, in original order)
/// offset N+M  tileset entries (20 bytes each, in original order)
/// ```
///
/// The header is 20 bytes — the trailing 4 bytes at offsets 16..20 hold
/// `files_offset` (the offset of the entries table). The original BIFF
/// readers tolerate any data placement before the entries table, so the
/// packing choice here is purely internal.
fn build_biff_v1(bif: &Bif) -> io::Result<Vec<u8>> {
    let files: Vec<&BifEmbeddedResource> = bif
        .resources
        .iter()
        .filter(|r| matches!(r, BifEmbeddedResource::File { .. }))
        .collect();
    let tilesets: Vec<&BifEmbeddedResource> = bif
        .resources
        .iter()
        .filter(|r| matches!(r, BifEmbeddedResource::Tileset { .. }))
        .collect();

    // Header (20 bytes) + payload + entries table.
    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(BIFFV1_SIGNATURE.as_bytes());
    out.extend_from_slice(&(files.len() as u32).to_le_bytes());
    out.extend_from_slice(&(tilesets.len() as u32).to_le_bytes());
    // Placeholder for files_offset; patched after we know the payload size.
    let files_offset_field = out.len();
    out.extend_from_slice(&0u32.to_le_bytes());

    // Pack each resource's bytes, in iteration order over `bif.resources`,
    // recording the new offset for each one. The importer reads files
    // first then tilesets, so when we write the entries table we need to
    // emit them in that split order — but the bytes themselves can be in
    // any order in the payload section as long as the table's offsets
    // point to the right places.
    let mut new_offsets: Vec<u32> = Vec::with_capacity(bif.resources.len());
    let mut reader = bif.datasource.reader()?;
    for resource in &bif.resources {
        let new_offset = u32::try_from(out.len())
            .map_err(|_| io::Error::other("BIFF V1 archive exceeds 4 GiB"))?;
        new_offsets.push(new_offset);
        let (offset, payload_size) = match resource {
            BifEmbeddedResource::File { offset, size, .. } => (*offset, *size as u64),
            BifEmbeddedResource::Tileset {
                offset,
                size,
                count,
                ..
            } => (*offset, *size as u64 * *count as u64),
        };
        reader.set_position(offset)?;
        let mut take = (&mut reader).take(payload_size);
        let mut payload = Vec::with_capacity(payload_size as usize);
        io::copy(&mut take, &mut payload)?;
        if (payload.len() as u64) != payload_size {
            return Err(io::Error::other(format!(
                "short read from BIF datasource: wanted {payload_size} bytes, got {}",
                payload.len()
            )));
        }
        out.extend_from_slice(&payload);
    }

    let files_offset = u32::try_from(out.len())
        .map_err(|_| io::Error::other("BIFF V1 entries table beyond 4 GiB"))?;
    out[files_offset_field..files_offset_field + 4].copy_from_slice(&files_offset.to_le_bytes());

    // File entries (16 bytes each), in original resource-vec order.
    for (i, resource) in bif.resources.iter().enumerate() {
        if let BifEmbeddedResource::File {
            locator,
            size,
            r#type,
            ..
        } = resource
        {
            // BIF file entries use the 16-bit type id; the
            // standalone-file resource kinds (`Sav` / `Tlk`) have
            // none and cannot live inside a BIF.
            let type_id = r#type.to_u16().ok_or_else(|| {
                io::Error::other(format!(
                    "BIF file entry #{i} has type {:?} which has no KEY/BIF u16 id and cannot be exported",
                    r#type,
                ))
            })?;
            out.extend_from_slice(&locator.to_le_bytes());
            out.extend_from_slice(&new_offsets[i].to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&type_id.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // unknown / reserved
        }
    }

    // Tileset entries (20 bytes each), in original resource-vec order.
    for (i, resource) in bif.resources.iter().enumerate() {
        if let BifEmbeddedResource::Tileset {
            locator,
            size,
            count,
            r#type,
            ..
        } = resource
        {
            let type_id = r#type.to_u16().ok_or_else(|| {
                io::Error::other(format!(
                    "BIF tileset entry #{i} has type {:?} which has no KEY/BIF u16 id and cannot be exported",
                    r#type,
                ))
            })?;
            out.extend_from_slice(&locator.to_le_bytes());
            out.extend_from_slice(&new_offsets[i].to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&type_id.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // unknown / reserved
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BifImporter;
    use infinitier_datasource::{DataSource, Importer, SeekExt};
    use infinitier_test_utils::get_assets_path;

    /// Compares two `Bif`s for functional equality: same `name`, `type`,
    /// same resource metadata (locator/size/count/type — *not* offset,
    /// which legitimately changes when the layout is repacked) and same
    /// per-resource byte payloads.
    fn assert_bifs_functionally_equal(a: &Bif, b: &Bif) {
        assert_eq!(a.name, b.name, "name");
        assert_eq!(a.r#type, b.r#type, "type");
        assert_eq!(a.resources.len(), b.resources.len(), "resource count");
        let mut a_reader = a.datasource.reader().unwrap();
        let mut b_reader = b.datasource.reader().unwrap();
        for (i, (ra, rb)) in a.resources.iter().zip(b.resources.iter()).enumerate() {
            match (ra, rb) {
                (
                    BifEmbeddedResource::File {
                        locator: la,
                        size: sa,
                        offset: oa,
                        r#type: ta,
                    },
                    BifEmbeddedResource::File {
                        locator: lb,
                        size: sb,
                        offset: ob,
                        r#type: tb,
                    },
                ) => {
                    assert_eq!(la, lb, "resource[{i}] locator");
                    assert_eq!(sa, sb, "resource[{i}] size");
                    assert_eq!(ta, tb, "resource[{i}] type");
                    let mut buf_a = vec![0u8; *sa as usize];
                    let mut buf_b = vec![0u8; *sb as usize];
                    a_reader.set_position(*oa).unwrap();
                    a_reader.read_exact(&mut buf_a).unwrap();
                    b_reader.set_position(*ob).unwrap();
                    b_reader.read_exact(&mut buf_b).unwrap();
                    assert_eq!(buf_a, buf_b, "resource[{i}] bytes");
                }
                (
                    BifEmbeddedResource::Tileset {
                        locator: la,
                        size: sa,
                        count: ca,
                        offset: oa,
                        r#type: ta,
                    },
                    BifEmbeddedResource::Tileset {
                        locator: lb,
                        size: sb,
                        count: cb,
                        offset: ob,
                        r#type: tb,
                    },
                ) => {
                    assert_eq!(la, lb, "tileset[{i}] locator");
                    assert_eq!(sa, sb, "tileset[{i}] size");
                    assert_eq!(ca, cb, "tileset[{i}] count");
                    assert_eq!(ta, tb, "tileset[{i}] type");
                    let total = (*sa as u64) * (*ca as u64);
                    let mut buf_a = vec![0u8; total as usize];
                    let mut buf_b = vec![0u8; total as usize];
                    a_reader.set_position(*oa).unwrap();
                    a_reader.read_exact(&mut buf_a).unwrap();
                    b_reader.set_position(*ob).unwrap();
                    b_reader.read_exact(&mut buf_b).unwrap();
                    assert_eq!(buf_a, buf_b, "tileset[{i}] bytes");
                }
                _ => panic!("resource[{i}] variant mismatch"),
            }
        }
    }

    fn roundtrip(path: &std::path::Path, name: &str) -> (Bif, Bif) {
        let source = DataSource::new(path);
        let original = BifImporter { name }.import(&source).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        BifExporter.export(&original, &mut buf).unwrap();
        let re_imported = BifImporter { name }.import(&DataSource::new(buf)).unwrap();
        (original, re_imported)
    }

    #[test]
    fn test_roundtrip_biff_v1() {
        // pst/CS_0511.bif: uncompressed BIFF V1, 4 file resources, no tilesets.
        let path = get_assets_path().join("KEY/pst/CS_0511.bif");
        let (original, re_imported) = roundtrip(&path, "biff_rt");
        assert_eq!(re_imported.r#type, Type::Biff);
        assert_bifs_functionally_equal(&original, &re_imported);
    }

    #[test]
    fn test_roundtrip_biff_v1_with_tileset() {
        // bg2_ee/data/area500c.bif: BIFF V1 mixing files and a tileset.
        let path = get_assets_path().join("KEY/bg2_ee/data/area500c.bif");
        let (original, re_imported) = roundtrip(&path, "biff_tile_rt");
        assert_eq!(re_imported.r#type, Type::Biff);
        assert_bifs_functionally_equal(&original, &re_imported);
    }

    #[test]
    fn test_roundtrip_bif_v1_0() {
        // iwd/CD2/Data/AR3603.cbf: single-zlib-stream BIF V1.0.
        let path = get_assets_path().join("KEY/iwd/CD2/Data/AR3603.cbf");
        let (original, re_imported) = roundtrip(&path, "bif_rt");
        assert_eq!(re_imported.r#type, Type::Bif);
        assert_bifs_functionally_equal(&original, &re_imported);
    }

    #[test]
    fn test_roundtrip_bifc_v1_0() {
        // bg2/data/Data/AREA070C.bif: chunked-zlib BIFC V1.0.
        let path = get_assets_path().join("KEY/bg2/data/Data/AREA070C.bif");
        let (original, re_imported) = roundtrip(&path, "bifc_rt");
        assert_eq!(re_imported.r#type, Type::Bifc);
        assert_bifs_functionally_equal(&original, &re_imported);
    }

    #[test]
    fn test_export_to_file_roundtrip() {
        let path = get_assets_path().join("KEY/pst/CS_0511.bif");
        let original = BifImporter { name: "file_rt" }
            .import(&DataSource::new(path.as_path()))
            .unwrap();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        BifExporter.export_to_file(&original, tmp.path()).unwrap();
        let re_imported = BifImporter { name: "file_rt" }
            .import(&DataSource::new(tmp.path().to_path_buf()))
            .unwrap();
        assert_bifs_functionally_equal(&original, &re_imported);
    }
}
