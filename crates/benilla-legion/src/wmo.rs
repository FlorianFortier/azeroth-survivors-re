//! Legion's buildings (WMO, version 17 like 1.12.1's) as 1.12.1's reader takes them. Most chunks
//! are already 1.12.1's and stay as they are; what changes:
//!
//! - the root's names: each texture (`MOTX`, which the materials' `MOMT` name by offset) becomes
//!   the library's `Legion\<file data id>.blp`, each doodad (`MODN`, named by `MODD`'s offsets) its
//!   `Legion\<file data id>.mdx`, and the skybox (`MOSB`) none; a name whose data the install
//!   lacks is left empty;
//! - a group's second and third texture coordinates and second vertex colours (Legion's extra
//!   `MOTV` and `MOCV`, which 1.12.1's reader would append to the first) are left out;
//! - a render batch whose material index is Legion's large one (`MOBA` flag `0x2`, a `u16` at
//!   `+0x0A`) takes it at `+0x17`, where 1.12.1 reads it.

use crate::terrain::Resolve;

/// A chunk's reversed magic and payload, in order.
fn chunks(b: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= b.len() {
        let mut magic: [u8; 4] = b[at..at + 4].try_into().unwrap();
        magic.reverse();
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        let end = (at + 8 + size).min(b.len());
        out.push((magic, &b[at + 8..end]));
        at = at.saturating_add(8 + size);
    }
    out
}

fn put(out: &mut Vec<u8>, magic: &[u8; 4], data: &[u8]) {
    let mut m = *magic;
    m.reverse();
    out.extend_from_slice(&m);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

/// Whether `b` is a group file (it holds `MOGP`) rather than a root.
pub fn is_group(b: &[u8]) -> bool {
    chunks(b).iter().any(|(m, _)| m == b"MOGP")
}

/// A name table rewritten: each name by `rename` (empty for none), and each old offset to the
/// new one.
fn rename_table(blob: &[u8], rename: impl Fn(&str) -> String) -> (Vec<u8>, Vec<(u32, u32)>) {
    let (mut out, mut moved) = (Vec::new(), Vec::new());
    let mut start = 0;
    for (i, &c) in blob.iter().enumerate() {
        if c != 0 {
            continue;
        }
        if i > start {
            let name = String::from_utf8_lossy(&blob[start..i]);
            moved.push((start as u32, out.len() as u32));
            out.extend_from_slice(rename(&name).as_bytes());
            out.push(0);
        }
        start = i + 1;
    }
    // 1.12.1's tables end on a NUL and pad to four.
    out.push(0);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    (out, moved)
}

/// An old offset's new one; one that named nothing points at the table's last NUL.
fn moved(table: &[(u32, u32)], old: u32, empty: u32) -> u32 {
    table
        .iter()
        .find(|(o, _)| *o == old)
        .map_or(empty, |(_, n)| *n)
}

/// A Legion WMO root with its names the library's.
pub fn root(b: &[u8], resolve: &dyn Resolve) -> Vec<u8> {
    let cs = chunks(b);
    let blob = |m: &[u8; 4]| cs.iter().find(|(c, _)| c == m).map_or(&[][..], |c| c.1);
    let (motx, textures) = rename_table(blob(b"MOTX"), |n| {
        resolve
            .file_id(n)
            .map_or_else(String::new, |id| format!("Legion\\{id}.blp"))
    });
    let (modn, models) = rename_table(blob(b"MODN"), |n| {
        resolve
            .file_id(&n.replace(".mdx", ".m2").replace(".MDX", ".m2"))
            .map_or_else(String::new, |id| format!("Legion\\{id}.mdx"))
    });
    let (no_texture, no_model) = (motx.len() as u32 - 1, modn.len() as u32 - 1);
    let mut out = Vec::new();
    for (magic, data) in &cs {
        match magic {
            b"MOTX" => put(&mut out, magic, &motx),
            b"MODN" => put(&mut out, magic, &modn),
            b"MOSB" => put(&mut out, magic, &[0; 4]),
            b"MOMT" => {
                let mut m = data.to_vec();
                for rec in m.as_chunks_mut::<64>().0 {
                    for at in [12, 24, 36] {
                        let old = u32::from_le_bytes(rec[at..at + 4].try_into().unwrap());
                        // Texture 2 and 3 may be unset (0): only names move.
                        if at != 12 && old == 0 {
                            continue;
                        }
                        let new = moved(&textures, old, no_texture);
                        rec[at..at + 4].copy_from_slice(&new.to_le_bytes());
                    }
                }
                put(&mut out, magic, &m);
            }
            b"MODD" => {
                let mut d = data.to_vec();
                for rec in d.as_chunks_mut::<40>().0 {
                    let old = u32::from_le_bytes(rec[0..4].try_into().unwrap()) & 0x00ff_ffff;
                    let new = moved(&models, old, no_model);
                    rec[0..4].copy_from_slice(&new.to_le_bytes());
                }
                put(&mut out, magic, &d);
            }
            _ => put(&mut out, magic, data),
        }
    }
    out
}

/// A Legion WMO group as 1.12.1's reader takes it.
pub fn group(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for (magic, data) in chunks(b) {
        if &magic != b"MOGP" || data.len() < 68 {
            put(&mut out, &magic, data);
            continue;
        }
        let mut mogp = data[..68].to_vec();
        let (mut uvs, mut colours) = (false, false);
        for (m, s) in chunks(&data[68..]) {
            match &m {
                b"MOTV" if uvs => continue,
                b"MOTV" => uvs = true,
                b"MOCV" if colours => continue,
                b"MOCV" => colours = true,
                _ => {}
            }
            if &m == b"MOBA" {
                let mut batches = s.to_vec();
                for rec in batches.as_chunks_mut::<24>().0 {
                    if rec[22] & 0x2 != 0 {
                        let material = u16::from_le_bytes([rec[10], rec[11]]);
                        rec[23] = material.min(255) as u8;
                        rec[22] &= !0x2;
                    }
                }
                put(&mut mogp, &m, &batches);
            } else {
                put(&mut mogp, &m, s);
            }
        }
        put(&mut out, b"MOGP", &mogp);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_roots_names_become_the_librarys_and_its_offsets_follow() {
        let motx = b"\0\0\0\0A.BLP\0\0\0B.BLP\0".to_vec();
        let mut momt = vec![0u8; 128];
        momt[12..16].copy_from_slice(&4u32.to_le_bytes());
        momt[64 + 12..64 + 16].copy_from_slice(&12u32.to_le_bytes());
        momt[64 + 24..64 + 28].copy_from_slice(&4u32.to_le_bytes());
        let modn = b"x.m2\0y.m2\0".to_vec();
        let mut modd = vec![0u8; 80];
        modd[40..44].copy_from_slice(&(5u32 | 0x0100_0000).to_le_bytes());
        let mut b = Vec::new();
        put(&mut b, b"MVER", &17u32.to_le_bytes());
        put(&mut b, b"MOTX", &motx);
        put(&mut b, b"MOMT", &momt);
        put(&mut b, b"MOSB", b"sky\0");
        put(&mut b, b"MODN", &modn);
        put(&mut b, b"MODD", &modd);
        let resolve = |p: &str| match p {
            "A.BLP" => Some(10),
            "B.BLP" => Some(11),
            "y.m2" => Some(20),
            _ => None,
        };
        let out = root(&b, &resolve);
        let cs = chunks(&out);
        let get = |m: &[u8; 4]| cs.iter().find(|(c, _)| c == m).unwrap().1;
        let motx = get(b"MOTX");
        let name = |o: usize| {
            let end = motx[o..].iter().position(|&c| c == 0).unwrap() + o;
            String::from_utf8_lossy(&motx[o..end]).into_owned()
        };
        let momt = get(b"MOMT");
        let at = |o: usize| u32::from_le_bytes(momt[o..o + 4].try_into().unwrap()) as usize;
        assert_eq!(name(at(12)), "Legion\\10.blp");
        assert_eq!(name(at(64 + 12)), "Legion\\11.blp");
        assert_eq!(name(at(64 + 24)), "Legion\\10.blp");
        assert_eq!(get(b"MOSB"), &[0; 4]);
        // The doodad the install lacks names nothing; the other its own.
        let modn = get(b"MODN");
        let modd = get(b"MODD");
        let off = |i: usize| u32::from_le_bytes(modd[i * 40..i * 40 + 4].try_into().unwrap());
        assert_eq!(modn[off(0) as usize], 0);
        assert!(modn[off(1) as usize..].starts_with(b"Legion\\20.mdx\0"));
    }

    #[test]
    fn a_group_keeps_one_set_of_coordinates_and_its_large_materials() {
        let mut batch = vec![0u8; 24];
        batch[10..12].copy_from_slice(&3u16.to_le_bytes());
        batch[22] = 0x2;
        let mut sub = Vec::new();
        put(&mut sub, b"MOTV", &[1; 8]);
        put(&mut sub, b"MOBA", &batch);
        put(&mut sub, b"MOTV", &[2; 8]);
        put(&mut sub, b"MOCV", &[3; 4]);
        put(&mut sub, b"MOCV", &[4; 4]);
        let mut mogp = vec![0u8; 68];
        mogp.extend(sub);
        let mut b = Vec::new();
        put(&mut b, b"MVER", &17u32.to_le_bytes());
        put(&mut b, b"MOGP", &mogp);
        assert!(is_group(&b));
        let out = group(&b);
        let cs = chunks(&out);
        let inner = chunks(&cs[1].1[68..]);
        let magics: Vec<&[u8; 4]> = inner.iter().map(|(m, _)| m).collect();
        assert_eq!(magics, [b"MOTV", b"MOBA", b"MOCV"]);
        assert_eq!(inner[0].1, &[1; 8]);
        assert_eq!((inner[1].1[22], inner[1].1[23]), (0, 3));
    }
}
