//! The local indices, `Data/data/<bucket><version>.idx`: sixteen buckets, each kept in its
//! newest version, mapping an encoded key's first nine bytes to where the file lies. A file's
//! version 7 layout: a header block (its size and hash, then the version, bucket and field
//! widths), padded to 16 bytes; the entries block (its size and hash, then 18-byte entries: the
//! key, a 5-byte big-endian archive and offset, a 4-byte little-endian size).

use std::collections::HashMap;

use crate::{bad, Result, Source};

/// Each stored file's own header in a data archive: its key reversed, its size, flags and checks.
pub const STORED_HEADER: usize = 30;

/// Where one encoded key's file lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub archive: u16,
    pub offset: u64,
    pub size: u32,
}

/// The newest `.idx` of each bucket, from the names the install's file list gives.
pub fn newest(names: &[String]) -> Vec<String> {
    let mut best: HashMap<String, String> = HashMap::new();
    for n in names
        .iter()
        .filter(|n| n.ends_with(".idx") && n.len() == 14)
    {
        let bucket = n[..2].to_string();
        if best.get(&bucket).is_none_or(|b| n[2..10] > b[2..10]) {
            best.insert(bucket, n.clone());
        }
    }
    let mut out: Vec<String> = best.into_values().collect();
    out.sort();
    out
}

/// One `.idx` file's entries.
pub fn parse(data: &[u8], into: &mut HashMap<[u8; 9], Entry>) -> Result<()> {
    let u32le = |at: usize| -> Result<u32> {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| bad("idx truncated"))
    };
    let header_size = u32le(0)? as usize;
    let fields = data
        .get(8..8 + header_size)
        .ok_or_else(|| bad("idx header truncated"))?;
    let version = u16::from_le_bytes([fields[0], fields[1]]);
    let (size_bytes, offset_bytes, key_bytes, offset_bits) = (
        fields[4] as usize,
        fields[5] as usize,
        fields[6] as usize,
        u32::from(fields[7]),
    );
    if version != 7 || key_bytes != 9 || offset_bytes != 5 || size_bytes != 4 {
        return Err(bad(format!(
            "idx v{version}, key {key_bytes}, offset {offset_bytes}, size {size_bytes}"
        )));
    }
    let entries_at = (8 + header_size).next_multiple_of(16);
    let entries_size = u32le(entries_at)? as usize;
    let start = entries_at + 8;
    let body = data
        .get(start..start + entries_size)
        .ok_or_else(|| bad("idx entries truncated"))?;
    for e in body.as_chunks::<18>().0 {
        let mut key = [0u8; 9];
        key.copy_from_slice(&e[..9]);
        let packed = e[9..14].iter().fold(0u64, |a, &b| a << 8 | u64::from(b));
        let mask = (1u64 << offset_bits) - 1;
        let entry = Entry {
            archive: (packed >> offset_bits) as u16,
            offset: packed & mask,
            size: u32::from_le_bytes([e[14], e[15], e[16], e[17]]),
        };
        into.entry(key).or_insert(entry);
    }
    Ok(())
}

/// Every bucket's newest index in `Data/data/`.
pub fn load<S: Source>(source: &S) -> Result<HashMap<[u8; 9], Entry>> {
    let names = source.list("Data/data")?;
    let mut out = HashMap::new();
    for name in newest(&names) {
        parse(&source.read_all(&format!("Data/data/{name}"))?, &mut out)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_bucket_keeps_its_newest_file() {
        let names: Vec<String> = [
            "000000004c.idx",
            "000000004d.idx",
            "0100000048.idx",
            "shmem",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(newest(&names), ["000000004d.idx", "0100000048.idx"]);
    }

    #[test]
    fn an_entry_splits_its_archive_and_offset() {
        // Header block: 16 bytes of fields; then entries at 0x28.
        let mut d = vec![0u8; 0x28];
        d[0..4].copy_from_slice(&16u32.to_le_bytes());
        d[8..16].copy_from_slice(&[7, 0, 0, 0, 4, 5, 9, 30]);
        d[0x20..0x24].copy_from_slice(&18u32.to_le_bytes());
        let mut e = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];
        // Archive 3, offset 0x1234: 3 << 30 | 0x1234 over five bytes.
        let packed: u64 = 3 << 30 | 0x1234;
        e.extend_from_slice(&packed.to_be_bytes()[3..]);
        e.extend_from_slice(&100u32.to_le_bytes());
        d.extend_from_slice(&e);
        let mut out = HashMap::new();
        parse(&d, &mut out).unwrap();
        assert_eq!(
            out[&[1, 2, 3, 4, 5, 6, 7, 8, 9]],
            Entry {
                archive: 3,
                offset: 0x1234,
                size: 100
            }
        );
    }
}
