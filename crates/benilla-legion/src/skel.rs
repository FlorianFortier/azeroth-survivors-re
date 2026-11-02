//! A `.skel` file: from 7.3 an HD model's skeleton, out of its `.m2` (whose `SKID` chunk names
//! it, the MD20's bone and sequence arrays left empty). Its chunks hold what the MD20 header would:
//!
//! - `SKS1`: the global loops, the sequences and their lookup;
//! - `SKB1`: the bones and the key bone lookup;
//! - `SKA1`: the attachments and their lookup;
//! - `AFID`: the `.anim` files of the sequences kept out, as a model's `AFID` names them; an
//!   `.anim` of a skeleton keeps its bones' keys in an `AFSB` chunk ([`anim_bones`]);
//! - `SKPD`: a parent skeleton, whose chunks stand in for those the child lacks.
//!
//! Each chunk's offsets count from the chunk's own start. [`merge`] lays them back into the MD20
//! as a model of its own holds them, so the transcoder reads an HD model like any other.

use crate::{u32_at, Error, Result};

/// A `.skel` file's chunks, by id.
#[derive(Debug, Default)]
pub struct Skel {
    pub sequences: Option<Vec<u8>>,
    pub bones: Option<Vec<u8>>,
    pub attachments: Option<Vec<u8>>,
    /// `(animation id, variation, file data id)`, as [`crate::md21::Md21::anims`].
    pub anims: Vec<(u16, u16, u32)>,
    /// The parent skeleton's file data id (`SKPD`).
    pub parent: Option<u32>,
}

impl Skel {
    /// The parent's chunks where this one lacks them.
    pub fn with_parent(mut self, parent: Skel) -> Skel {
        self.sequences = self.sequences.or(parent.sequences);
        self.bones = self.bones.or(parent.bones);
        self.attachments = self.attachments.or(parent.attachments);
        if self.anims.is_empty() {
            self.anims = parent.anims;
        }
        self
    }
}

pub fn parse(file: &[u8]) -> Result<Skel> {
    let mut out = Skel::default();
    let mut at = 0;
    let mut skeleton = false;
    while at + 8 <= file.len() {
        let id: [u8; 4] = file[at..at + 4].try_into().unwrap();
        let size = u32_at(file, at + 4).ok_or(Error::Truncated("skel chunk"))? as usize;
        let body = file
            .get(at + 8..at + 8 + size)
            .ok_or(Error::Truncated("skel chunk body"))?;
        match &id {
            b"SKL1" => skeleton = true,
            b"SKS1" => out.sequences = Some(body.to_vec()),
            b"SKB1" => out.bones = Some(body.to_vec()),
            b"SKA1" => out.attachments = Some(body.to_vec()),
            b"SKPD" => out.parent = u32_at(body, 8).filter(|&p| p != 0),
            b"AFID" => out.anims = crate::md21::anim_ids(body),
            _ => {}
        }
        at += 8 + size;
    }
    if !skeleton {
        return Err(Error::Format("no SKL1 chunk: not a skeleton".into()));
    }
    Ok(out)
}

/// The MD20 header's arrays each chunk fills: `(the chunk's array at, the header's at)`.
const SEQUENCE_ARRAYS: [(usize, usize); 3] = [(0, 0x14), (8, 0x1c), (16, 0x24)];
const BONE_ARRAYS: [(usize, usize); 2] = [(0, 0x2c), (8, 0x34)];
const ATTACHMENT_ARRAYS: [(usize, usize); 2] = [(0, 0xf0), (8, 0xf8)];

/// An MD20 with its skeleton's chunks laid in after it, the header's arrays pointing at them and
/// every offset inside moved with them: a bone's three tracks, and their keys for the sequences
/// kept in the file (sequence flag `0x20`) or on a global loop. The keys of a sequence kept in an
/// `.anim` stay as they are, counted in its file's `AFSB` chunk.
pub fn merge(md20: &[u8], skel: &Skel) -> Result<Vec<u8>> {
    if md20.get(0..4) != Some(b"MD20") {
        return Err(Error::Format("not MD20".into()));
    }
    let mut out = md20.to_vec();
    let lay = |out: &mut Vec<u8>, chunk: &[u8], arrays: &[(usize, usize)]| -> Result<usize> {
        while !out.len().is_multiple_of(16) {
            out.push(0);
        }
        let base = out.len();
        out.extend_from_slice(chunk);
        for &(from, to) in arrays {
            let count = u32_at(chunk, from).ok_or(Error::Truncated("skel array"))?;
            let ofs = u32_at(chunk, from + 4).ok_or(Error::Truncated("skel array"))?;
            let ofs = if count == 0 { 0 } else { ofs + base as u32 };
            out[to..to + 4].copy_from_slice(&count.to_le_bytes());
            out[to + 4..to + 8].copy_from_slice(&ofs.to_le_bytes());
        }
        Ok(base)
    };
    // Which sequences keep their keys in the file.
    let mut in_file = Vec::new();
    if let Some(s) = &skel.sequences {
        lay(&mut out, s, &SEQUENCE_ARRAYS)?;
        let n = u32_at(s, 8).ok_or(Error::Truncated("skel sequences"))? as usize;
        let at = u32_at(s, 12).ok_or(Error::Truncated("skel sequences"))? as usize;
        for i in 0..n {
            let flags = u32_at(s, at + i * 64 + 12).ok_or(Error::Truncated("skel sequence"))?;
            in_file.push(flags & 0x20 != 0);
        }
    }
    if let Some(b) = &skel.bones {
        let base = lay(&mut out, b, &BONE_ARRAYS)?;
        let n = u32_at(b, 0).ok_or(Error::Truncated("skel bones"))? as usize;
        let at = base + u32_at(b, 4).ok_or(Error::Truncated("skel bones"))? as usize;
        for i in 0..n {
            for track in [16, 36, 56] {
                move_track(&mut out, at + i * 88 + track, base, &in_file)?;
            }
        }
    }
    if let Some(a) = &skel.attachments {
        let base = lay(&mut out, a, &ATTACHMENT_ARRAYS)?;
        let n = u32_at(a, 0).ok_or(Error::Truncated("skel attachments"))? as usize;
        let at = base + u32_at(a, 4).ok_or(Error::Truncated("skel attachments"))? as usize;
        for i in 0..n {
            move_track(&mut out, at + i * 40 + 20, base, &in_file)?;
        }
    }
    Ok(out)
}

/// A 20-byte track at `at` whose offsets count from `base`: its two arrays of arrays moved, and
/// the inner arrays of the sequences whose keys are in the file (all of them on a global loop).
fn move_track(out: &mut [u8], at: usize, base: usize, in_file: &[bool]) -> Result<()> {
    let global = out
        .get(at + 2..at + 4)
        .map(|g| u16::from_le_bytes([g[0], g[1]]) != 0xffff)
        .ok_or(Error::Truncated("track"))?;
    for array in [at + 4, at + 12] {
        let count = u32_at(out, array).ok_or(Error::Truncated("track"))? as usize;
        if count == 0 {
            continue;
        }
        let outer = u32_at(out, array + 4).ok_or(Error::Truncated("track"))? as usize + base;
        out[array + 4..array + 8].copy_from_slice(&(outer as u32).to_le_bytes());
        for j in 0..count {
            let inner = outer + j * 8;
            let n = u32_at(out, inner).ok_or(Error::Truncated("track keys"))?;
            if n == 0 || !(global || in_file.get(j).copied().unwrap_or(false)) {
                continue;
            }
            let ofs = u32_at(out, inner + 4).ok_or(Error::Truncated("track keys"))? as usize;
            out[inner + 4..inner + 8].copy_from_slice(&((ofs + base) as u32).to_le_bytes());
        }
    }
    Ok(())
}

/// A skeleton's `.anim` file as the transcoder reads one: its bones' keys (`AFSB`, whose offsets
/// count from its own start) as an `AFM2` chunk. A file without `AFSB` is returned as it is.
pub fn anim_bones(file: &[u8]) -> Vec<u8> {
    let mut at = 0;
    while at + 8 <= file.len() {
        let size = u32_at(file, at + 4).unwrap_or(0) as usize;
        if &file[at..at + 4] == b"AFSB" {
            if let Some(body) = file.get(at + 8..at + 8 + size) {
                let mut out = Vec::with_capacity(body.len() + 8);
                out.extend_from_slice(b"AFM2");
                out.extend_from_slice(&(body.len() as u32).to_le_bytes());
                out.extend_from_slice(body);
                return out;
            }
        }
        at += 8 + size;
    }
    file.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunk(out: &mut Vec<u8>, id: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(id);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
    }

    fn put(b: &mut [u8], at: usize, v: u32) {
        b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// A skeleton of one bone whose rotation is keyed in two sequences, the first in the file, the
    /// second in an `.anim`; one sequence's keys each.
    fn skel() -> Vec<u8> {
        // SKS1: no globals, two sequences at 32, no lookup.
        let mut sks = vec![0u8; 32 + 128];
        put(&mut sks, 8, 2);
        put(&mut sks, 12, 32);
        put(&mut sks, 32 + 12, 0x20);
        // SKB1: one bone at 16, its rotation's timestamps at 104 (two inner arrays), the first
        // sequence's keys at 120.
        let mut skb = vec![0u8; 128];
        put(&mut skb, 0, 1);
        put(&mut skb, 4, 16);
        let bone = 16;
        skb[bone + 36 + 2..bone + 36 + 4].copy_from_slice(&0xffffu16.to_le_bytes());
        put(&mut skb, bone + 36 + 4, 2);
        put(&mut skb, bone + 36 + 8, 104);
        put(&mut skb, 104, 1);
        put(&mut skb, 108, 120);
        put(&mut skb, 112, 1);
        put(&mut skb, 116, 40);
        let mut f = Vec::new();
        chunk(&mut f, b"SKL1", &[0; 16]);
        chunk(&mut f, b"SKS1", &sks);
        chunk(&mut f, b"SKB1", &skb);
        chunk(&mut f, b"AFID", &[5, 0, 0, 0, 9, 0, 0, 0]);
        f
    }

    #[test]
    fn a_skeleton_lands_in_its_model_with_its_offsets() {
        let s = parse(&skel()).unwrap();
        assert_eq!(s.anims, [(5, 0, 9)]);
        let mut md20 = vec![0u8; 0x130];
        md20[0..4].copy_from_slice(b"MD20");
        let m = merge(&md20, &s).unwrap();
        let at = |o: usize| u32_at(&m, o).unwrap() as usize;
        // The sequences and the bone where the header looks.
        assert_eq!(at(0x1c), 2);
        assert_eq!(u32_at(&m, at(0x20) + 12), Some(0x20));
        assert_eq!(at(0x2c), 1);
        let bone = at(0x30);
        let base = bone - 16;
        // The rotation's timestamps moved, the in-file sequence's keys too, the other's not.
        let outer = at(bone + 36 + 8);
        assert_eq!(outer, base + 104);
        assert_eq!(at(outer + 4), base + 120);
        assert_eq!(at(outer + 12), 40);
        assert_eq!(at(0x34), 0);
    }

    #[test]
    fn a_parent_stands_in_for_what_its_child_lacks() {
        let mut child = Vec::new();
        chunk(&mut child, b"SKL1", &[0; 16]);
        let mut pd = vec![0u8; 16];
        put(&mut pd, 8, 77);
        chunk(&mut child, b"SKPD", &pd);
        let c = parse(&child).unwrap();
        assert_eq!(c.parent, Some(77));
        let c = c.with_parent(parse(&skel()).unwrap());
        assert!(c.sequences.is_some() && c.bones.is_some());
        assert_eq!(c.anims, [(5, 0, 9)]);
        assert!(parse(b"MD20").is_err());
    }

    #[test]
    fn a_skeletons_anim_reads_as_its_bones_keys() {
        let mut f = Vec::new();
        chunk(&mut f, b"AFM2", &[1, 2]);
        chunk(&mut f, b"AFSB", &[3, 4, 5]);
        assert_eq!(
            anim_bones(&f),
            [b"AFM2".as_slice(), &[3, 0, 0, 0, 3, 4, 5]].concat()
        );
        assert_eq!(anim_bones(b"raw"), b"raw");
    }
}
