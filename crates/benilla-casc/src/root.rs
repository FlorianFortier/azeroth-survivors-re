//! Legion's root file (7.x, before the `MFST` header 8.2 brings): blocks, each a count, content
//! flags and locale flags, the count's file data ids as deltas (the first the id itself, each next
//! one the previous plus one plus its delta), then the count's records: a content key and the
//! Jenkins hash of the file's path.

use std::collections::HashMap;

use crate::{bad, Result};

/// The locale flags root tags enUS files with.
pub const LOCALE_EN_US: u32 = 0x2;

pub struct Root {
    /// File data id → (locale flags, content key), one per block that holds the id.
    by_id: HashMap<u32, Vec<(u32, [u8; 16])>>,
    by_hash: HashMap<u64, u32>,
}

impl Root {
    pub fn parse(d: &[u8]) -> Result<Self> {
        let u32le = |at: usize| -> Result<u32> {
            d.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| bad("root truncated"))
        };
        if d.get(0..4) == Some(b"TSFM") {
            return Err(bad("an 8.2+ root (MFST): not Legion's"));
        }
        let mut by_id: HashMap<u32, Vec<(u32, [u8; 16])>> = HashMap::new();
        let mut by_hash = HashMap::new();
        let mut at = 0;
        while at + 12 <= d.len() {
            let count = u32le(at)? as usize;
            let locale = u32le(at + 8)?;
            at += 12;
            let mut ids = Vec::with_capacity(count);
            let mut next: i64 = 0;
            for i in 0..count {
                let delta = u32le(at + i * 4)? as i32;
                let id = next + i64::from(delta);
                ids.push(id as u32);
                next = id + 1;
            }
            at += count * 4;
            for (i, &id) in ids.iter().enumerate() {
                let r = at + i * 24;
                let rec = d
                    .get(r..r + 24)
                    .ok_or_else(|| bad("root record truncated"))?;
                let mut ckey = [0u8; 16];
                ckey.copy_from_slice(&rec[..16]);
                let hash = u64::from_le_bytes(rec[16..24].try_into().unwrap());
                by_id.entry(id).or_default().push((locale, ckey));
                by_hash.entry(hash).or_insert(id);
            }
            at += count * 24;
        }
        Ok(Self { by_id, by_hash })
    }

    /// A file's content key in `locale`, else in any locale (most files are for all).
    pub fn by_id(&self, id: u32, locale: u32) -> Option<[u8; 16]> {
        let all = self.by_id.get(&id)?;
        all.iter()
            .find(|(l, _)| l & locale != 0)
            .or(all.first())
            .map(|(_, k)| *k)
    }

    pub fn by_hash(&self, hash: u64) -> Option<u32> {
        self.by_hash.get(&hash).copied()
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// Every file data id, for a probe.
    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.by_id.keys().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_deltas_from_the_one_before() {
        let mut d = Vec::new();
        for v in [3u32, 0, LOCALE_EN_US] {
            d.extend_from_slice(&v.to_le_bytes());
        }
        for delta in [10i32, 0, 5] {
            d.extend_from_slice(&delta.to_le_bytes());
        }
        for k in 0..3u8 {
            d.extend_from_slice(&[k; 16]);
            d.extend_from_slice(&u64::from(k).to_le_bytes());
        }
        let r = Root::parse(&d).unwrap();
        assert_eq!(r.by_id(10, LOCALE_EN_US), Some([0; 16]));
        assert_eq!(r.by_id(11, LOCALE_EN_US), Some([1; 16]));
        assert_eq!(r.by_id(17, LOCALE_EN_US), Some([2; 16]));
        assert_eq!(r.by_hash(2), Some(17));
    }
}
