//! Legion's terrain as 1.12.1's: a map's WDT, and each tile's split files (the root's heights and
//! normals, `_tex0`'s textures, layers, alpha and shadow, `_obj0`'s doodads) written back as one
//! monolithic 1.12.1 ADT, the form the client's terrain reader takes.
//!
//! What 1.12.1 has no room for is left out: the vertex colours (`MCCV`), the texture scales and
//! height textures (`MAMP`, `MTXP`), the far doodads (`_obj1`), the level of detail (`_lod`). The
//! alpha is decoded whatever its form (Legion's big alpha, 8-bit, raw or run-length) and written
//! run-length, which the 1.12.1 reader takes in either alpha mode; a high-resolution hole map
//! becomes the 4×4 one (a block open where any of its four cells is). The liquids (`MH2O`) are
//! not carried yet.
//!
//! Names become the library's: a texture `Legion\<file data id>.blp`, a model
//! `Legion\<file data id>.mdx` (the client reads `.mdx` names as `.m2`), resolved by `resolve`; a
//! model or texture the install does not name is dropped (a texture for a 1.12.1 one); a building
//! `Legion\<its own path>`, which the library answers as 1.12.1's ([`crate::wmo`]).

use crate::{Error, Result};

/// One IFF chunk: reversed magic, size, payload.
struct Chunk<'a> {
    magic: [u8; 4],
    data: &'a [u8],
}

/// The chunks of `b`, in order, with their magics read forward ("MCNK").
fn chunks(b: &[u8]) -> Vec<Chunk<'_>> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= b.len() {
        let mut magic: [u8; 4] = b[at..at + 4].try_into().unwrap();
        magic.reverse();
        let size = u32::from_le_bytes(b[at + 4..at + 8].try_into().unwrap()) as usize;
        let end = (at + 8 + size).min(b.len());
        out.push(Chunk {
            magic,
            data: &b[at + 8..end],
        });
        at = at + 8 + size;
    }
    out
}

/// The first chunk `m` among `cs`.
fn find<'a>(cs: &[Chunk<'a>], m: &[u8; 4]) -> Option<&'a [u8]> {
    cs.iter().find(|c| &c.magic == m).map(|c| c.data)
}

/// A table of `u32`s.
fn u32s(b: Option<&[u8]>) -> Vec<u32> {
    b.unwrap_or(&[])
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    b.get(at..at + 4)
        .map_or(0, |s| u32::from_le_bytes(s.try_into().unwrap()))
}

/// Write one chunk: `magic` forward, stored reversed.
fn put(out: &mut Vec<u8>, magic: &[u8; 4], data: &[u8]) {
    let mut m = *magic;
    m.reverse();
    out.extend_from_slice(&m);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
}

/// `MPHD` flags: the map's alpha maps are 8-bit (`0x4`, or `0x80`, height texturing, which
/// implies it).
const BIG_ALPHA: u32 = 0x4 | 0x80;

/// A Legion WDT as 1.12.1's: its tile table, its flags dropped (none of them 1.12.1's own).
pub fn wdt(legion: &[u8]) -> Result<Vec<u8>> {
    let main = chunks(legion)
        .into_iter()
        .find(|c| &c.magic == b"MAIN")
        .ok_or_else(|| Error::Format("WDT: no MAIN".into()))?;
    let mut out = Vec::new();
    put(&mut out, b"MVER", &18u32.to_le_bytes());
    put(&mut out, b"MPHD", &[0u8; 32]);
    put(&mut out, b"MAIN", main.data);
    Ok(out)
}

/// Whether a Legion WDT's tiles carry 8-bit alpha.
pub fn big_alpha(legion_wdt: &[u8]) -> bool {
    chunks(legion_wdt)
        .into_iter()
        .find(|c| &c.magic == b"MPHD")
        .is_some_and(|c| u32_at(c.data, 0) & BIG_ALPHA != 0)
}

/// The 64×64 opacity of one alpha layer at `at` in `mcal`, in whatever form `flags` and
/// `big_alpha` say: run-length (`0x200`), 8-bit, or 4-bit.
fn decode_alpha(mcal: &[u8], at: usize, flags: u32, big_alpha: bool) -> [u8; 4096] {
    let mut out = [0u8; 4096];
    let src = mcal.get(at..).unwrap_or(&[]);
    if flags & 0x200 != 0 {
        let (mut i, mut n) = (0, 0);
        while n < 4096 && i < src.len() {
            let token = src[i];
            i += 1;
            let count = usize::from(token & 0x7f);
            if token & 0x80 != 0 {
                let v = src.get(i).copied().unwrap_or(0);
                i += 1;
                for _ in 0..count.min(4096 - n) {
                    out[n] = v;
                    n += 1;
                }
            } else {
                for _ in 0..count.min(4096 - n) {
                    out[n] = src.get(i).copied().unwrap_or(0);
                    i += 1;
                    n += 1;
                }
            }
        }
    } else if big_alpha {
        let k = src.len().min(4096);
        out[..k].copy_from_slice(&src[..k]);
    } else {
        for (i, &b) in src.iter().take(2048).enumerate() {
            out[i * 2] = (b & 0x0f) * 17;
            out[i * 2 + 1] = (b >> 4) * 17;
        }
    }
    out
}

/// A 64×64 opacity run-length coded: a fill token (`0x80 | count`, value) for a run of three or
/// more, else a copy token (`count`, values), runs at most 127 long.
fn encode_alpha(a: &[u8; 4096]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut copy: Vec<u8> = Vec::new();
    let flush = |out: &mut Vec<u8>, copy: &mut Vec<u8>| {
        for part in copy.chunks(127) {
            out.push(part.len() as u8);
            out.extend_from_slice(part);
        }
        copy.clear();
    };
    while i < a.len() {
        let run = a[i..].iter().take(127).take_while(|&&v| v == a[i]).count();
        if run >= 3 {
            flush(&mut out, &mut copy);
            out.push(0x80 | run as u8);
            out.push(a[i]);
            i += run;
        } else {
            copy.push(a[i]);
            i += 1;
        }
    }
    flush(&mut out, &mut copy);
    out
}

/// The 4×4 hole map of a high-resolution one (eight rows of eight bits): a block of 2×2 cells is
/// open where any of them is.
fn low_res_holes(high: &[u8]) -> u16 {
    let mut holes = 0u16;
    for row in 0..8 {
        let bits = high.get(row).copied().unwrap_or(0);
        for col in 0..8 {
            if bits & (1 << col) != 0 {
                holes |= 1 << ((row / 2) * 4 + col / 2);
            }
        }
    }
    holes
}

/// MCNK header flags 1.12.1 reads the same: a shadow map (`0x1`), impassable (`0x2`). The liquid
/// bits stay clear: no `MCLQ` is written.
const KEPT_FLAGS: u32 = 0x1 | 0x2;
/// Legion's: the hole map is the high-resolution one, in the header's height and normal offsets.
const HIGH_RES_HOLES: u32 = 0x10000;

/// The tables a tile's names resolve through: a path in the install to its file data id.
pub trait Resolve {
    fn file_id(&self, path: &str) -> Option<u32>;
}

impl<F: Fn(&str) -> Option<u32>> Resolve for F {
    fn file_id(&self, path: &str) -> Option<u32> {
        self(path)
    }
}

/// A 1.12.1 texture for one the install lacks.
const MISSING_TEXTURE: &str = "Tileset\\Generic\\Black.blp";

/// The NUL-separated names of a string table, by their offsets.
fn names(blob: &[u8]) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, &c) in blob.iter().enumerate() {
        if c == 0 {
            if i > start {
                out.push((
                    start as u32,
                    String::from_utf8_lossy(&blob[start..i]).into_owned(),
                ));
            }
            start = i + 1;
        }
    }
    out
}

/// A Legion tile (`root`, `tex0` and, when there, `obj0`) as one 1.12.1 ADT.
pub fn adt(
    root: &[u8],
    tex0: &[u8],
    obj0: Option<&[u8]>,
    big_alpha: bool,
    resolve: &dyn Resolve,
) -> Result<Vec<u8>> {
    let root_chunks = chunks(root);
    let tex_chunks = chunks(tex0);
    let obj_chunks = obj0.map(chunks).unwrap_or_default();

    // Textures, in MTEX order so the layers' indices hold.
    let mtex: Vec<String> = find(&tex_chunks, b"MTEX")
        .map(names)
        .unwrap_or_default()
        .into_iter()
        .map(|(_, n)| match resolve.file_id(&n) {
            Some(id) => format!("Legion\\{id}.blp"),
            None => MISSING_TEXTURE.to_string(),
        })
        .collect();

    // Models: each placement's name through MMID; a model the install lacks drops its
    // placements, the rest renumbered.
    let mmdx = find(&obj_chunks, b"MMDX").map(names).unwrap_or_default();
    let mmid = u32s(find(&obj_chunks, b"MMID"));
    let mut models: Vec<String> = Vec::new();
    // Legion's MMID index to ours.
    let mut model_of: Vec<Option<u32>> = Vec::new();
    for &ofs in &mmid {
        let name = mmdx
            .iter()
            .find(|(o, _)| *o == ofs)
            .map(|(_, n)| n.as_str());
        let id = name.and_then(|n| resolve.file_id(&n.replace(".mdx", ".m2")));
        model_of.push(id.map(|id| {
            let path = format!("Legion\\{id}.mdx");
            match models.iter().position(|m| *m == path) {
                Some(i) => i as u32,
                None => {
                    models.push(path);
                    models.len() as u32 - 1
                }
            }
        }));
    }
    // Legion's MDDF index to ours.
    let mut placement_of: Vec<Option<u32>> = Vec::new();
    let mut mddf = Vec::new();
    for r in find(&obj_chunks, b"MDDF")
        .unwrap_or(&[])
        .as_chunks::<36>()
        .0
    {
        let ours = model_of.get(u32_at(r, 0) as usize).copied().flatten();
        placement_of.push(ours.map(|m| {
            let mut rec = r.to_vec();
            rec[0..4].copy_from_slice(&m.to_le_bytes());
            // Legion's flags past 1.12.1's (a file data id name, a scale beyond) are cleared.
            rec[34..36].copy_from_slice(&(u16::from_le_bytes([r[34], r[35]]) & 0x1).to_le_bytes());
            mddf.extend_from_slice(&rec);
            (mddf.len() / 36 - 1) as u32
        }));
    }

    // Buildings likewise: each named by its own path under the library's (`Legion\<path>`),
    // which answers its root and groups as 1.12.1's ([`crate::wmo`]).
    let mwmo = find(&obj_chunks, b"MWMO").map(names).unwrap_or_default();
    let mut wmos: Vec<String> = Vec::new();
    let mut wmo_of: Vec<Option<u32>> = Vec::new();
    for &ofs in &u32s(find(&obj_chunks, b"MWID")) {
        let name = mwmo
            .iter()
            .find(|(o, _)| *o == ofs)
            .map(|(_, n)| n.as_str())
            .filter(|n| resolve.file_id(n).is_some());
        wmo_of.push(name.map(|n| {
            let path = format!("Legion\\{n}");
            match wmos.iter().position(|m| *m == path) {
                Some(i) => i as u32,
                None => {
                    wmos.push(path);
                    wmos.len() as u32 - 1
                }
            }
        }));
    }
    let mut wmo_placement_of: Vec<Option<u32>> = Vec::new();
    let mut modf = Vec::new();
    for r in find(&obj_chunks, b"MODF")
        .unwrap_or(&[])
        .as_chunks::<64>()
        .0
    {
        let ours = wmo_of.get(u32_at(r, 0) as usize).copied().flatten();
        wmo_placement_of.push(ours.map(|w| {
            let mut rec = r.to_vec();
            rec[0..4].copy_from_slice(&w.to_le_bytes());
            // Legion's flags past 1.12.1's (a level of detail, a scale, a file data id name)
            // are cleared, and its scale with them.
            rec[56..58].copy_from_slice(&(u16::from_le_bytes([r[56], r[57]]) & 0x1).to_le_bytes());
            rec[62..64].copy_from_slice(&[0, 0]);
            modf.extend_from_slice(&rec);
            (modf.len() / 64 - 1) as u32
        }));
    }

    let mut out = Vec::new();
    put(&mut out, b"MVER", &18u32.to_le_bytes());
    put(&mut out, b"MHDR", &[0u8; 64]);
    let mut blob = Vec::new();
    for t in &mtex {
        blob.extend_from_slice(t.as_bytes());
        blob.push(0);
    }
    put(&mut out, b"MTEX", &blob);
    let (mut blob, mut ids) = (Vec::new(), Vec::new());
    for m in &models {
        ids.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        blob.extend_from_slice(m.as_bytes());
        blob.push(0);
    }
    put(&mut out, b"MMDX", &blob);
    put(&mut out, b"MMID", &ids);
    let (mut blob, mut ids) = (Vec::new(), Vec::new());
    for w in &wmos {
        ids.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        blob.extend_from_slice(w.as_bytes());
        blob.push(0);
    }
    put(&mut out, b"MWMO", &blob);
    put(&mut out, b"MWID", &ids);
    put(&mut out, b"MDDF", &mddf);
    put(&mut out, b"MODF", &modf);

    let root_mcnk: Vec<&[u8]> = root_chunks
        .iter()
        .filter(|c| &c.magic == b"MCNK")
        .map(|c| c.data)
        .collect();
    let tex_mcnk: Vec<&[u8]> = tex_chunks
        .iter()
        .filter(|c| &c.magic == b"MCNK")
        .map(|c| c.data)
        .collect();
    let obj_mcnk: Vec<&[u8]> = obj_chunks
        .iter()
        .filter(|c| &c.magic == b"MCNK")
        .map(|c| c.data)
        .collect();
    if root_mcnk.len() != 256 {
        return Err(Error::Format(format!(
            "ADT: {} terrain chunks",
            root_mcnk.len()
        )));
    }
    for (i, r) in root_mcnk.iter().enumerate() {
        let tex = tex_mcnk.get(i).copied().unwrap_or(&[]);
        let obj = obj_mcnk.get(i).copied().unwrap_or(&[]);
        out.extend(mcnk(
            r,
            tex,
            obj,
            big_alpha,
            &placement_of,
            &wmo_placement_of,
        )?);
    }
    Ok(out)
}

/// One terrain chunk, its header first, its sub-chunks after at the offsets the header gives
/// (counted from the MCNK's magic, as 1.12.1's are).
fn mcnk(
    root: &[u8],
    tex: &[u8],
    obj: &[u8],
    big_alpha: bool,
    placement_of: &[Option<u32>],
    wmo_placement_of: &[Option<u32>],
) -> Result<Vec<u8>> {
    let header = root
        .get(..128)
        .ok_or_else(|| Error::Format("MCNK: short header".into()))?;
    let flags = u32_at(header, 0);
    let sub = chunks(&root[128..]);
    let heights = find(&sub, b"MCVT").unwrap_or(&[]);
    let mut normals = find(&sub, b"MCNR").unwrap_or(&[]).to_vec();
    normals.resize(448, 0);
    let tex_sub = chunks(tex);
    let mcly = find(&tex_sub, b"MCLY").unwrap_or(&[]);
    let mcal = find(&tex_sub, b"MCAL").unwrap_or(&[]);
    let shadow = find(&tex_sub, b"MCSH");

    // Layers: the base as it is, each over it with its alpha run-length coded anew.
    let (mut layers, mut alpha) = (Vec::new(), Vec::new());
    for (i, l) in mcly.as_chunks::<16>().0.iter().take(4).enumerate() {
        let (texture, lflags, at) = (u32_at(l, 0), u32_at(l, 4), u32_at(l, 8) as usize);
        let mut rec = [0u8; 16];
        rec[0..4].copy_from_slice(&texture.to_le_bytes());
        if i > 0 {
            let a = decode_alpha(mcal, at, lflags, big_alpha);
            rec[4..8].copy_from_slice(&(0x100u32 | 0x200).to_le_bytes());
            rec[8..12].copy_from_slice(&(alpha.len() as u32).to_le_bytes());
            alpha.extend(encode_alpha(&a));
        }
        layers.extend_from_slice(&rec);
    }

    // Doodad references, then building references, through the placements kept.
    let obj_sub = chunks(obj);
    let kept = |refs: Option<&[u8]>, of: &[Option<u32>]| -> Vec<u32> {
        u32s(refs)
            .into_iter()
            .filter_map(|i| of.get(i as usize).copied().flatten())
            .collect()
    };
    let doodad_refs = kept(find(&obj_sub, b"MCRD"), placement_of);
    let wmo_refs = kept(find(&obj_sub, b"MCRW"), wmo_placement_of);
    let refs: Vec<u8> = doodad_refs
        .iter()
        .chain(&wmo_refs)
        .flat_map(|r| r.to_le_bytes())
        .collect();

    let holes = if flags & HIGH_RES_HOLES != 0 {
        low_res_holes(&header[0x14..0x1c])
    } else {
        u32_at(header, 0x3c) as u16
    };

    // The sub-chunks, each placed after the header, their offsets from the MCNK's magic.
    let mut body = Vec::new();
    let at = |magic: &[u8; 4], data: &[u8], body: &mut Vec<u8>| -> u32 {
        let ofs = 8 + 128 + body.len() as u32;
        put(body, magic, data);
        ofs
    };
    let ofs_height = at(b"MCVT", heights, &mut body);
    let ofs_normal = at(b"MCNR", &normals, &mut body);
    let ofs_layer = at(b"MCLY", &layers, &mut body);
    let ofs_refs = at(b"MCRF", &refs, &mut body);
    let ofs_alpha = at(b"MCAL", &alpha, &mut body);
    let (ofs_shadow, size_shadow) = match shadow {
        Some(s) => (at(b"MCSH", s, &mut body), s.len() as u32),
        None => (0, 0),
    };
    let ofs_sound = at(b"MCSE", &[], &mut body);

    let mut h = [0u8; 128];
    let mut set = |o: usize, v: u32| h[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let kept = (flags & KEPT_FLAGS) | if size_shadow > 0 { 0x1 } else { 0 };
    set(0x00, kept);
    set(0x04, u32_at(header, 0x04));
    set(0x08, u32_at(header, 0x08));
    set(0x0c, (layers.len() / 16) as u32);
    set(0x10, doodad_refs.len() as u32);
    set(0x14, ofs_height);
    set(0x18, ofs_normal);
    set(0x1c, ofs_layer);
    set(0x20, ofs_refs);
    set(0x24, ofs_alpha);
    set(0x28, alpha.len() as u32);
    set(0x2c, ofs_shadow);
    set(0x30, size_shadow);
    // Legion's area ids are not 1.12.1's: none.
    set(0x34, 0);
    set(0x38, wmo_refs.len() as u32);
    set(0x3c, u32::from(holes));
    set(0x58, ofs_sound);
    // The chunk's corner, as Legion's.
    h[0x68..0x74].copy_from_slice(&header[0x68..0x74]);

    let mut out = Vec::new();
    let mut data = h.to_vec();
    data.extend(body);
    put(&mut out, b"MCNK", &data);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_alpha_layer_comes_back_from_its_run_length_coding() {
        let mut a = [0u8; 4096];
        for (i, v) in a.iter_mut().enumerate() {
            *v = if i < 300 { 255 } else { (i % 7) as u8 * 30 };
        }
        let coded = encode_alpha(&a);
        assert!(coded.len() < 4096 + 4096 / 127 + 1);
        assert_eq!(decode_alpha(&coded, 0, 0x200, false), a);
        // 4-bit and 8-bit layers read as they are.
        assert_eq!(decode_alpha(&[0x2f], 0, 0, false)[..2], [255, 34]);
        assert_eq!(decode_alpha(&[9, 8], 0, 0, true)[..2], [9, 8]);
    }

    #[test]
    fn a_high_resolution_hole_opens_its_block() {
        // Row 0, column 3: block (0, 1). Row 7, column 7: block (3, 3).
        let mut high = [0u8; 8];
        high[0] = 1 << 3;
        high[7] = 1 << 7;
        assert_eq!(low_res_holes(&high), (1 << 1) | (1 << 15));
    }

    #[test]
    fn a_wdt_keeps_its_tiles_and_drops_its_flags() {
        let mut legion = Vec::new();
        put(&mut legion, b"MVER", &18u32.to_le_bytes());
        put(&mut legion, b"MPHD", &0x1cau32.to_le_bytes());
        let mut main = vec![0u8; 64 * 64 * 8];
        main[(30 * 64 + 24) * 8] = 1;
        put(&mut legion, b"MAIN", &main);
        assert!(big_alpha(&legion));
        let w = wdt(&legion).unwrap();
        let cs = chunks(&w);
        assert_eq!(u32_at(cs[1].data, 0), 0);
        assert_eq!(cs[2].data, &main[..]);
    }
}
