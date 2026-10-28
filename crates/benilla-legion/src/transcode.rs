//! A Legion model rewritten as a 1.12.1 one: an MD20 version 256 file with its skin profile inside,
//! which 1.12.1's reader (`benilla-m2`) and renderer take as they are.
//!
//! What it carries: the vertices, one skin profile (its sections cut back to 1.12.1's 32 bytes,
//! the batches of the sections the display hides dropped), the textures named by the paths the
//! caller picks (a replaceable one named, and made a type 0, once its display gives its file),
//! the materials, the bones in their bind pose (no keys yet: step 4 lays Legion's tracks onto
//! 1.12.1's timeline), the sequences on one timeline, the colours and texture weights at their
//! first key, the attachments, the lookups and the bounds. Texture transforms, events, lights,
//! cameras, ribbons and particles are left out. Legion's shader combos come down to one texture a
//! batch, its base, and its blend modes to 1.12.1's.
//!
//! The 1.12.1 header (`0x144` bytes) and the Legion one (version 272 to 274) differ by the playable
//! animation lookup (`+0x2c`, 1.12.1 only), the embedded views where Legion counts its skin files
//! (`+0x44`), the texture flipbooks (`+0x6c`, 1.12.1 only), and an `M2Track` (28 bytes in 1.12.1,
//! 20 in Legion: the per-sequence ranges gone, the keys an array per sequence).

use crate::display::CreatureDisplay;
use crate::model::Model;
use crate::{array_at, u32_at, Error, Result};

/// The 1.12.1 header's size.
pub const HEADER: usize = 0x144;
/// An `M2Track` in 1.12.1: interpolation, global sequence, ranges, timestamps, values.
const TRACK: usize = 28;

/// The path a Legion texture is named by in a transcoded model: its file data id under `Legion\`,
/// which no 1.12.1 file uses.
pub fn virtual_path(file: u32) -> String {
    format!("Legion\\{file}.blp")
}

/// The file each texture record shows: `TXID`'s, else the file the record's path names
/// (`id_of`), else, for a replaceable one, the display's. `None` leaves the record as it is.
pub fn texture_files(
    model: &Model,
    txid: &[u32],
    display: Option<&CreatureDisplay>,
    id_of: impl Fn(&str) -> Option<u32>,
) -> Vec<Option<u32>> {
    model
        .textures
        .iter()
        .enumerate()
        .map(|(i, t)| {
            txid.get(i)
                .copied()
                .filter(|&f| f != 0)
                .or_else(|| {
                    (!t.filename.is_empty())
                        .then(|| id_of(&t.filename))
                        .flatten()
                })
                .or_else(|| display.and_then(|d| d.texture_for(t.kind)))
        })
        .collect()
}

/// Rewrite the Legion model `md20` (an MD21 chunk's body) and its skin profile `skin` (a `.skin`
/// file) as a 1.12.1 MD20. `textures` names each texture record (see [`texture_files`] and
/// [`virtual_path`]); `shows` says which skin sections (by geoset id) are drawn.
pub fn to_vanilla(
    md20: &[u8],
    skin: &[u8],
    textures: &[Option<String>],
    shows: impl Fn(u16) -> bool,
) -> Result<Vec<u8>> {
    if md20.get(0..4) != Some(b"MD20") {
        return Err(Error::Format("not MD20".into()));
    }
    let version = u32_at(md20, 4).ok_or(Error::Truncated("version"))?;
    if !(264..=274).contains(&version) {
        return Err(Error::Format(format!("MD20 version {version}")));
    }
    if skin.get(0..4) != Some(b"SKIN") {
        return Err(Error::Format("not SKIN".into()));
    }
    let src = Src(md20);
    let mut out = Out::new();
    out.b[0..4].copy_from_slice(b"MD20");
    out.put_u32(4, 256);
    // The model's flags: the tilts; Legion's later bits (blend map overrides at 0x8…) are not
    // 1.12.1's.
    out.put_u32(0x10, src.u32(0x10)? & 0x7);

    out.copy(&src, 0x08, 0x08, 1)?; // name
    out.copy(&src, 0x14, 0x14, 4)?; // global sequences
    out.copy(&src, 0x24, 0x24, 2)?; // animation lookup
    out.copy(&src, 0x34, 0x3c, 2)?; // key bone lookup
    out.copy(&src, 0x3c, 0x44, 48)?; // vertices: the same 48 bytes
    out.copy(&src, 0x68, 0x7c, 2)?; // replaceable texture lookup
    out.copy(&src, 0x78, 0x8c, 2)?; // bone lookup
    out.copy(&src, 0x80, 0x94, 2)?; // texture lookup
    out.copy(&src, 0x88, 0x9c, 2)?; // texture unit lookup
    out.copy(&src, 0x90, 0xa4, 2)?; // transparency lookup
    out.copy(&src, 0xd8, 0xec, 2)?; // collision triangles
    out.copy(&src, 0xe0, 0xf4, 12)?; // collision vertices
    out.copy(&src, 0xe8, 0xfc, 12)?; // collision normals
    out.copy(&src, 0xf8, 0x10c, 2)?; // attachment lookup
                                     // Bounds, then the collision box: two boxes and radii, 28 bytes each, back to back.
    let bounds = md20.get(0xa0..0xd8).ok_or(Error::Truncated("bounds"))?;
    out.b[0xb4..0xec].copy_from_slice(bounds);

    // Sequences: Legion's own durations, laid one after another on one timeline.
    let (seqs, n) = src.records(0x1c, 64)?;
    let mut recs = Vec::with_capacity(n * 0x44);
    let mut clock = 0u32;
    for s in seqs.as_chunks::<64>().0 {
        let duration = le32(s, 4);
        let (start, end) = (clock, clock.saturating_add(duration));
        clock = end.saturating_add(1);
        recs.extend_from_slice(&s[0..4]); // id, variation
        recs.extend_from_slice(&start.to_le_bytes());
        recs.extend_from_slice(&end.to_le_bytes());
        recs.extend_from_slice(&s[8..12]); // move speed
                                           // 0x20, the data in the model file, as every 1.12.1 sequence's is.
        recs.extend_from_slice(&(le32(s, 12) | 0x20).to_le_bytes());
        recs.extend_from_slice(&s[16..28]); // frequency, pad, replay
        let blend = u32::from(u16::from_le_bytes([s[28], s[29]]));
        recs.extend_from_slice(&blend.to_le_bytes());
        recs.extend_from_slice(&s[32..64]); // bounds, radius, next variation, alias
    }
    out.array(0x1c, n, &recs);

    // Bones: 88 bytes to 108, in their bind pose.
    let (bones, n) = src.records(0x2c, 88)?;
    let mut recs = Vec::with_capacity(n * 108);
    for b in bones.as_chunks::<88>().0 {
        recs.extend_from_slice(&b[0..12]); // key bone, flags, parent, submesh
        for _ in 0..3 {
            recs.extend_from_slice(&empty_track());
        }
        recs.extend_from_slice(&b[76..88]); // pivot
    }
    out.array(0x34, n, &recs);

    // The one skin profile.
    let view = transcode_skin(&mut out, skin, &shows)?;
    let at = out.append(&view);
    out.put_u32(0x4c, 1);
    out.put_u32(0x50, at);

    // Colours: an RGB track and an alpha one, each at its first key.
    let (_, n) = src.records(0x48, 40)?;
    let mut recs = Vec::with_capacity(n * 0x38);
    for i in 0..n {
        let base = src.offset(0x48)? + i * 40;
        let rgb = out.static_track(src.first_key(base, 12));
        let alpha = out.static_track(src.first_key(base + 20, 2));
        recs.extend_from_slice(&rgb);
        recs.extend_from_slice(&alpha);
    }
    out.array(0x54, n, &recs);

    // Texture weights, at their first key.
    let (_, n) = src.records(0x58, 20)?;
    let mut recs = Vec::with_capacity(n * TRACK);
    for i in 0..n {
        let base = src.offset(0x58)? + i * 20;
        recs.extend_from_slice(&out.static_track(src.first_key(base, 2)));
    }
    out.array(0x64, n, &recs);

    // Textures: a named one is a type 0 by its path; the rest keep their type, unnamed.
    let (texs, n) = src.records(0x50, 16)?;
    let mut recs = Vec::with_capacity(n * 16);
    for (i, t) in texs.as_chunks::<16>().0.iter().enumerate() {
        let name = textures.get(i).and_then(|t| t.as_deref());
        let (kind, name) = match name {
            Some(path) => {
                let mut z = path.as_bytes().to_vec();
                z.push(0);
                let at = out.append(&z);
                (0, (z.len() as u32, at))
            }
            None => (le32(t, 0), (0, 0)),
        };
        recs.extend_from_slice(&kind.to_le_bytes());
        recs.extend_from_slice(&t[4..8]); // wrap flags
        recs.extend_from_slice(&name.0.to_le_bytes());
        recs.extend_from_slice(&name.1.to_le_bytes());
    }
    out.array(0x5c, n, &recs);

    // Materials: the flags 1.12.1 knows (unlit, unfogged, two-sided, billboard, no depth write)
    // and the blend modes it has; Legion's 7 (`One, OneMinusSrcAlpha`, a premultiplied blend) is
    // nearest an alpha blend.
    let (mats, n) = src.records(0x70, 4)?;
    let mut recs = Vec::with_capacity(n * 4);
    for m in mats.as_chunks::<4>().0 {
        let flags = u16::from_le_bytes([m[0], m[1]]) & 0x1f;
        let blend = match u16::from_le_bytes([m[2], m[3]]) {
            b @ 0..=6 => b,
            _ => 2,
        };
        recs.extend_from_slice(&flags.to_le_bytes());
        recs.extend_from_slice(&blend.to_le_bytes());
    }
    out.array(0x84, n, &recs);

    // No texture transforms are carried: every batch's lookup says none.
    let (_, n) = src.records(0x98, 2)?;
    out.array(0xac, n, &vec![0xff; n * 2]);

    // Attachments: 40 bytes to 48, the bone widened, the visibility track left out.
    let (atts, n) = src.records(0xf0, 40)?;
    let mut recs = Vec::with_capacity(n * 48);
    for a in atts.as_chunks::<40>().0 {
        recs.extend_from_slice(&a[0..4]); // id
        let bone = u32::from(u16::from_le_bytes([a[4], a[5]]));
        recs.extend_from_slice(&bone.to_le_bytes());
        recs.extend_from_slice(&a[8..20]); // position
        recs.extend_from_slice(&empty_track());
    }
    out.array(0x104, n, &recs);

    Ok(out.b)
}

/// The skin profile as one 44-byte `M2View`, its arrays appended to `out`.
fn transcode_skin(out: &mut Out, skin: &[u8], shows: &dyn Fn(u16) -> bool) -> Result<Vec<u8>> {
    let s = Src(skin);
    let mut view = Vec::with_capacity(44);
    // The model vertices the skin's vertices are, the triangles, the bone properties: as they are.
    for (at, stride) in [(4, 2), (12, 2), (20, 4)] {
        let (bytes, n) = s.records(at, stride)?;
        let (count, ofs) = out.put_array(n, bytes);
        view.extend_from_slice(&count.to_le_bytes());
        view.extend_from_slice(&ofs.to_le_bytes());
    }
    // Sections: 48 bytes to 32, the starts' high bits (`level`) folded back, which 1.12.1 cannot
    // hold past 65535.
    let (secs, n) = s.records(28, 48)?;
    let mut recs = Vec::with_capacity(n * 32);
    let mut ids = Vec::with_capacity(n);
    for x in secs.as_chunks::<48>().0 {
        let level = u32::from(u16::from_le_bytes([x[2], x[3]])) << 16;
        let start = |at: usize| -> Result<[u8; 2]> {
            u16::try_from(u32::from(u16::from_le_bytes([x[at], x[at + 1]])) + level)
                .map(u16::to_le_bytes)
                .map_err(|_| Error::Format("a skin past 65535 vertices or indices".into()))
        };
        ids.push(u16::from_le_bytes([x[0], x[1]]));
        recs.extend_from_slice(&x[0..2]);
        recs.extend_from_slice(&[0, 0]);
        recs.extend_from_slice(&start(4)?);
        recs.extend_from_slice(&x[6..8]);
        recs.extend_from_slice(&start(8)?);
        recs.extend_from_slice(&x[10..32]);
    }
    let (count, ofs) = out.put_array(n, &recs);
    view.extend_from_slice(&count.to_le_bytes());
    view.extend_from_slice(&ofs.to_le_bytes());
    // Batches: the shown sections' only, one texture each.
    let (bats, _) = s.records(36, 24)?;
    let mut recs = Vec::new();
    let mut kept = 0;
    for b in bats.as_chunks::<24>().0 {
        let section = u16::from_le_bytes([b[4], b[5]]);
        if !ids.get(section as usize).is_some_and(|&id| shows(id)) {
            continue;
        }
        let mut b = *b;
        let count = u16::from_le_bytes([b[14], b[15]]).min(1);
        b[14..16].copy_from_slice(&count.to_le_bytes());
        recs.extend_from_slice(&b);
        kept += 1;
    }
    let (count, ofs) = out.put_array(kept, &recs);
    view.extend_from_slice(&count.to_le_bytes());
    view.extend_from_slice(&ofs.to_le_bytes());
    view.extend_from_slice(&s.u32(44)?.to_le_bytes()); // bones a draw holds at most
    Ok(view)
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// A track with no keys: no animation, the value the reader takes as none.
fn empty_track() -> [u8; TRACK] {
    let mut t = [0; TRACK];
    t[2..4].copy_from_slice(&0xffffu16.to_le_bytes());
    t
}

/// The source file.
struct Src<'a>(&'a [u8]);

impl<'a> Src<'a> {
    fn u32(&self, at: usize) -> Result<u32> {
        u32_at(self.0, at).ok_or(Error::Truncated("header"))
    }

    /// Where the array whose `M2Array` is at `header` starts.
    fn offset(&self, header: usize) -> Result<usize> {
        Ok(array_at(self.0, header)?.1)
    }

    /// The bytes and count of the array whose `M2Array` is at `header`.
    fn records(&self, header: usize, stride: usize) -> Result<(&'a [u8], usize)> {
        let (n, o) = array_at(self.0, header)?;
        let end = n
            .checked_mul(stride)
            .and_then(|l| l.checked_add(o))
            .ok_or(Error::Truncated("array"))?;
        let bytes = self.0.get(o..end).ok_or(Error::Truncated("array"))?;
        Ok((bytes, n))
    }

    /// The first key of the Legion track at `track` (20 bytes: interpolation, global sequence,
    /// then timestamps and values, an array per sequence): its `size` bytes.
    fn first_key(&self, track: usize, size: usize) -> Option<&'a [u8]> {
        let (n, o) = array_at(self.0, track + 12).ok()?;
        (0..n).find_map(|i| {
            let (count, at) = array_at(self.0, o + i * 8).ok()?;
            (count > 0).then(|| self.0.get(at..at + size)).flatten()
        })
    }
}

/// The file being written: the header, then each array, 16-byte aligned.
struct Out {
    b: Vec<u8>,
}

impl Out {
    fn new() -> Self {
        Self { b: vec![0; HEADER] }
    }

    fn put_u32(&mut self, at: usize, v: u32) {
        self.b[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    /// Append `bytes` and return their offset.
    fn append(&mut self, bytes: &[u8]) -> u32 {
        self.b.resize(self.b.len().next_multiple_of(16), 0);
        let at = self.b.len() as u32;
        self.b.extend_from_slice(bytes);
        at
    }

    /// Append an array of `n` records and return its `M2Array`; an empty one is `(0, 0)`.
    fn put_array(&mut self, n: usize, bytes: &[u8]) -> (u32, u32) {
        if n == 0 {
            (0, 0)
        } else {
            (n as u32, self.append(bytes))
        }
    }

    /// An array of `n` records, its `M2Array` written at `header`.
    fn array(&mut self, header: usize, n: usize, bytes: &[u8]) {
        let (count, at) = self.put_array(n, bytes);
        self.put_u32(header, count);
        self.put_u32(header + 4, at);
    }

    /// The Legion array at `from` copied as it is to the 1.12.1 header's `to`.
    fn copy(&mut self, src: &Src, from: usize, to: usize, stride: usize) -> Result<()> {
        let (bytes, n) = src.records(from, stride)?;
        self.array(to, n, bytes);
        Ok(())
    }

    /// A 1.12.1 track holding `key` from time 0 on, its key appended; none without one.
    fn static_track(&mut self, key: Option<&[u8]>) -> [u8; TRACK] {
        let mut t = empty_track();
        if let Some(value) = key {
            let times = self.append(&0u32.to_le_bytes());
            let values = self.append(value);
            for (at, v) in [(12, 1), (16, times), (20, 1), (24, values)] {
                t[at..at + 4].copy_from_slice(&v.to_le_bytes());
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::u16_at;
    use std::io::Cursor;

    /// A Legion model of one triangle in two sections (geosets 0 and 102), each with a batch, one
    /// bone, one sequence, one texture, a material, a colour at half alpha and an attachment.
    fn legion() -> (Vec<u8>, Vec<u8>) {
        let mut m = Out::new();
        m.b[0..4].copy_from_slice(b"MD20");
        m.put_u32(4, 274);
        m.put_u32(0x10, 0x8 | 0x1);
        m.array(0x08, 4, b"Imp\0");
        let mut seq = vec![0u8; 64];
        seq[0..2].copy_from_slice(&4u16.to_le_bytes()); // run
        seq[4..8].copy_from_slice(&800u32.to_le_bytes());
        seq[28..30].copy_from_slice(&150u16.to_le_bytes());
        m.array(0x1c, 1, &seq);
        m.array(
            0x24,
            5,
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0],
        );
        let mut bone = vec![0u8; 88];
        bone[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        bone[8..10].copy_from_slice(&(-1i16).to_le_bytes());
        bone[76..80].copy_from_slice(&1.5f32.to_le_bytes());
        m.array(0x2c, 1, &bone);
        let mut verts = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            let mut v = vec![0u8; 48];
            for (k, c) in p.iter().enumerate() {
                v[k * 4..k * 4 + 4].copy_from_slice(&c.to_le_bytes());
            }
            v[12] = 255;
            verts.extend_from_slice(&v);
        }
        m.array(0x3c, 3, &verts);
        m.put_u32(0x44, 1);
        // A colour: its alpha track has one key in its first sequence, 0x3fff.
        let alpha = m.append(&0x3fffu16.to_le_bytes());
        let time = m.append(&0u32.to_le_bytes());
        let inner_v = m.append(
            &[1, 0, 0, 0]
                .iter()
                .chain(&alpha.to_le_bytes())
                .copied()
                .collect::<Vec<u8>>(),
        );
        let inner_t = m.append(
            &[1, 0, 0, 0]
                .iter()
                .chain(&time.to_le_bytes())
                .copied()
                .collect::<Vec<u8>>(),
        );
        let mut color = vec![0u8; 40];
        color[22..24].copy_from_slice(&0xffffu16.to_le_bytes());
        color[24..28].copy_from_slice(&1u32.to_le_bytes());
        color[28..32].copy_from_slice(&inner_t.to_le_bytes());
        color[32..36].copy_from_slice(&1u32.to_le_bytes());
        color[36..40].copy_from_slice(&inner_v.to_le_bytes());
        m.array(0x48, 1, &color);
        let mut tex = vec![0u8; 16];
        tex[0..4].copy_from_slice(&11u32.to_le_bytes());
        tex[4..8].copy_from_slice(&3u32.to_le_bytes());
        m.array(0x50, 1, &tex);
        m.array(0x70, 1, &[0x40 | 0x4, 0, 7, 0]);
        m.array(0x78, 1, &[0, 0]);
        m.array(0x80, 1, &[0, 0]);
        m.array(0x88, 1, &[0, 0]);
        m.array(0x98, 1, &[0, 0]);
        let mut att = vec![0u8; 40];
        att[0..4].copy_from_slice(&17u32.to_le_bytes());
        att[16..20].copy_from_slice(&2.0f32.to_le_bytes());
        m.array(0xf0, 1, &att);
        let mut lookup = vec![0xffu8; 36];
        lookup[34..36].copy_from_slice(&0u16.to_le_bytes());
        m.array(0xf8, 18, &lookup);
        m.b[0xa0 + 24..0xa0 + 28].copy_from_slice(&2.0f32.to_le_bytes());

        let mut s = Out::new();
        s.b.truncate(48);
        s.b[0..4].copy_from_slice(b"SKIN");
        s.array(4, 3, &[0, 0, 1, 0, 2, 0]);
        s.array(12, 6, &[0, 0, 1, 0, 2, 0, 0, 0, 2, 0, 1, 0]);
        s.array(20, 3, &[0; 12]);
        let mut secs = Vec::new();
        for (id, first) in [(0u16, 0u16), (102, 3)] {
            let mut x = vec![0u8; 48];
            x[0..2].copy_from_slice(&id.to_le_bytes());
            x[6..8].copy_from_slice(&3u16.to_le_bytes());
            x[8..10].copy_from_slice(&first.to_le_bytes());
            x[10..12].copy_from_slice(&3u16.to_le_bytes());
            secs.extend_from_slice(&x);
        }
        s.array(28, 2, &secs);
        let mut bats = Vec::new();
        for section in [0u16, 1] {
            let mut b = vec![0u8; 24];
            b[2..4].copy_from_slice(&0x8003u16.to_le_bytes());
            b[4..6].copy_from_slice(&section.to_le_bytes());
            b[8..10].copy_from_slice(&0u16.to_le_bytes());
            b[14..16].copy_from_slice(&2u16.to_le_bytes());
            bats.extend_from_slice(&b);
        }
        s.array(36, 2, &bats);
        s.put_u32(44, 1);
        (m.b, s.b)
    }

    #[test]
    fn a_legion_model_reads_as_a_vanilla_one() {
        let (md20, skin) = legion();
        let names = [Some(virtual_path(1234))];
        let file = to_vanilla(&md20, &skin, &names, |id| id != 102).unwrap();
        let f = benilla_m2::parse_m2(&mut Cursor::new(&file[..])).unwrap();
        let m = f.model();
        assert_eq!(m.vertices.len(), 3);
        assert_eq!(m.vertices[1].position.x, 1.0);
        assert_eq!(m.bones.len(), 1);
        assert_eq!((m.bones[0].parent, m.bones[0].pivot.x), (-1, 1.5));
        assert_eq!(m.textures.len(), 1);
        assert_eq!(
            m.textures[0].filename.string.to_str().unwrap(),
            "Legion\\1234.blp"
        );
        assert!(m.textures[0].wrap_x && m.textures[0].wrap_y);
        assert_eq!(m.materials[0].blend_mode.bits(), 2);
        assert_eq!(m.materials[0].flags.bits(), 0x4);
        assert_eq!(m.texture_transform_lookup, [0xffff]);
        assert_eq!(
            m.color_alpha_tracks[0].constant(),
            Some(0x3fff as f32 / 32767.0)
        );
        assert_eq!(m.attachments.len(), 1);
        assert_eq!(m.attachments[0].id, 17);
        assert_eq!(m.attachments[0].position[2], 2.0);

        let s = m.parse_embedded_skin(&file, 0).unwrap();
        assert_eq!(s.indices().len(), 3);
        assert_eq!(s.triangles().len(), 6);
        assert_eq!(s.submeshes().len(), 2);
        assert_eq!(s.submeshes()[1].triangle_start, 3);
        // The hidden geoset's batch is gone; the kept one draws one texture.
        assert_eq!(s.batches().len(), 1);
        assert_eq!(s.batches()[0].skin_section_index, 0);
        assert_eq!(s.batches()[0].texture_count, 1);
    }

    #[test]
    fn sequences_lie_one_after_another() {
        let (md20, skin) = legion();
        let file = to_vanilla(&md20, &skin, &[None], |_| true).unwrap();
        let at = le32(&file, 0x20) as usize;
        assert_eq!(le32(&file, 0x1c), 1);
        assert_eq!(u16_at(&file, at), Some(4));
        assert_eq!((le32(&file, at + 4), le32(&file, at + 8)), (0, 800));
        assert_eq!(le32(&file, at + 0x10) & 0x20, 0x20);
        assert_eq!(le32(&file, at + 0x20), 150);
        // An unnamed texture keeps its type.
        assert_eq!(le32(&file, le32(&file, 0x60) as usize), 11);
        // The blend-override flag is not 1.12.1's.
        assert_eq!(le32(&file, 0x10), 0x1);
    }

    #[test]
    fn a_texture_comes_from_txid_its_path_or_its_display() {
        use crate::model::Texture;
        let tex = |kind, name: &str| Texture {
            kind,
            flags: 0,
            filename: name.into(),
        };
        let model = Model {
            version: 274,
            name: String::new(),
            sequences: 0,
            skin_profiles: 1,
            vertices: Vec::new(),
            textures: vec![tex(0, ""), tex(0, "a.blp"), tex(11, ""), tex(2, "")],
            materials: Vec::new(),
            bones: Vec::new(),
            texture_lookup: Vec::new(),
            bounds: ([0.0; 3], [0.0; 3]),
        };
        let display = CreatureDisplay {
            id: 1,
            model: 1,
            model_file: 1,
            textures: [77, 0, 0],
            geosets: 0,
            scale: 1.0,
            model_scale: 1.0,
            collision_height: 0.0,
        };
        let files = texture_files(&model, &[5], Some(&display), |p| {
            (p == "a.blp").then_some(6)
        });
        assert_eq!(files, [Some(5), Some(6), Some(77), None]);
    }
}
