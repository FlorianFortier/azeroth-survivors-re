//! The encoding file: content key → encoded keys. `EN`, a version, the key sizes, the page sizes
//! (KiB, big-endian) and counts, the encoding specs; then the content pages' index (first key
//! and checksum each) and the pages: entries of a key count, a 40-bit file size, the content key
//! and its encoded keys, until a count of 0 or the page's end.

use std::collections::HashMap;

use crate::{bad, Result};

pub struct Encoding {
    map: HashMap<[u8; 16], [u8; 16]>,
}

impl Encoding {
    pub fn parse(d: &[u8]) -> Result<Self> {
        if d.get(0..2) != Some(b"EN") || d.len() < 22 {
            return Err(bad("not an encoding file"));
        }
        let (ckey_size, ekey_size) = (d[3] as usize, d[4] as usize);
        if ckey_size != 16 || ekey_size != 16 {
            return Err(bad("encoding key sizes"));
        }
        let be16 = |at: usize| u16::from_be_bytes([d[at], d[at + 1]]) as usize;
        let be32 =
            |at: usize| u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]) as usize;
        let page_size = be16(5) * 1024;
        let page_count = be32(9);
        let espec_size = be32(18);
        let index_at = 22 + espec_size;
        let pages_at = index_at + page_count * 32;
        let mut map = HashMap::new();
        for p in 0..page_count {
            let start = pages_at + p * page_size;
            let page = d
                .get(start..start + page_size)
                .ok_or_else(|| bad("encoding page truncated"))?;
            let mut at = 0;
            while at + 6 + 16 <= page.len() {
                let keys = page[at] as usize;
                if keys == 0 {
                    break;
                }
                let ckey_at = at + 6;
                let ekey_at = ckey_at + 16;
                if ekey_at + 16 > page.len() {
                    break;
                }
                let mut ckey = [0u8; 16];
                let mut ekey = [0u8; 16];
                ckey.copy_from_slice(&page[ckey_at..ckey_at + 16]);
                ekey.copy_from_slice(&page[ekey_at..ekey_at + 16]);
                map.insert(ckey, ekey);
                at = ekey_at + keys * 16;
            }
        }
        Ok(Self { map })
    }

    /// A content key's first encoded key.
    pub fn ekey(&self, ckey: &[u8; 16]) -> Option<[u8; 16]> {
        self.map.get(ckey).copied()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}
