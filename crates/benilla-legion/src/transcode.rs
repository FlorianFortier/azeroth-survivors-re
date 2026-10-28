//! A Legion model rewritten as a 1.12.1 one: an MD20 version 256 file with its skin profile inside,
//! which 1.12.1's reader (`benilla-m2`) and renderer take as they are.
//!
//! What it carries: the vertices, one skin profile (its sections cut back to 1.12.1's 32 bytes,
//! the batches of the sections the display hides dropped), the textures named by the paths the
//! caller picks (a replaceable one named, and made a type 0, once its display gives its file),
//! the materials, the bones, colours and texture weights with their keys, the sequences on one
//! timeline, the attachments, the texture transforms, the particle and ribbon emitters
//! ([`particles`], [`ribbons`]), the lookups and the bounds. Events, lights and cameras are left
//! out. Legion's shader combos come down to one texture a batch, its base, and its blend modes to
//! 1.12.1's.
//!
//! The keys ([`Animation`]): a Legion track holds an array of keys per sequence, each timed from
//! its sequence's start, and a sequence's arrays may lie in an `.anim` file of its own (its flag
//! `0x20` clear); an alias (flag `0x40`) plays another sequence's keys. 1.12.1's track is one
//! array timed on the model's timeline, with a window of key indices per sequence: each
//! sequence's keys are moved to its place on that timeline, and a sequence with none for a track
//! that others animate gets one, its rest value (a bone's bind pose), as Legion means by none.
//! Rotations are Legion's compressed quaternions (four `i16`), widened to 1.12.1's four floats;
//! a spline track's tangents are dropped, and it plays linear. The animation lookup is rebuilt
//! direct (Legion's is a hash table), and the playable lookup made from `AnimationData`'s
//! fallbacks, so an animation the model lacks plays the one Legion would.
//!
//! The 1.12.1 header (`0x144` bytes) and the Legion one (version 272 to 274) differ by the playable
//! animation lookup (`+0x2c`, 1.12.1 only), the embedded views where Legion counts its skin files
//! (`+0x44`), the texture flipbooks (`+0x6c`, 1.12.1 only), and an `M2Track` (28 bytes in 1.12.1,
//! 20 in Legion: the per-sequence ranges gone, the keys an array per sequence).

use crate::model::Model;
use crate::{array_at, u16_at, u32_at, Error, Result};

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
/// (`id_of`), else, for a replaceable one, the file `replace` gives its type (a creature
/// display's skins for types 11 to 13, an item display's for type 2). `None` leaves the record
/// as it is.
pub fn texture_files(
    model: &Model,
    txid: &[u32],
    replace: impl Fn(u32) -> Option<u32>,
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
                .or_else(|| replace(t.kind))
        })
        .collect()
}

/// What the keys need beside the model.
#[derive(Default)]
pub struct Animation<'a> {
    /// The `.anim` files of the sequences kept out of the model: `(animation id, variation, the
    /// file)`, as the model's `AFID` chunk names them.
    pub files: &'a [(u16, u16, Vec<u8>)],
    /// `AnimationData`'s `Fallback`, by animation id: what plays for an animation the model does
    /// not have. Empty, the model has no playable lookup, and plays only what it holds.
    pub fallbacks: &'a [u16],
}

/// Rewrite the Legion model `md20` (an MD21 chunk's body) and its skin profile `skin` (a `.skin`
/// file) as a 1.12.1 MD20. `textures` names each texture record (see [`texture_files`] and
/// [`virtual_path`]); `shows` says which skin sections (by geoset id) are drawn; `animation`
/// brings the `.anim` files and the fallbacks.
pub fn to_vanilla(
    md20: &[u8],
    skin: &[u8],
    textures: &[Option<String>],
    shows: impl Fn(u16) -> bool,
    animation: &Animation,
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
    let seqs = seqs.as_chunks::<64>().0;
    let bands = bands(md20, seqs, animation.files);
    let mut recs = Vec::with_capacity(n * 0x44);
    for (i, (s, band)) in seqs.iter().zip(&bands).enumerate() {
        recs.extend_from_slice(&s[0..4]); // id, variation
        recs.extend_from_slice(&band.start.to_le_bytes());
        recs.extend_from_slice(&band.start.saturating_add(le32(s, 4)).to_le_bytes());
        recs.extend_from_slice(&s[8..12]); // move speed
                                           // 0x20, the keys in the model file, as every 1.12.1 sequence's are; an alias has its
                                           // own copy of its sequence's keys, so it is one no more.
        recs.extend_from_slice(&((le32(s, 12) | 0x20) & !0x40).to_le_bytes());
        recs.extend_from_slice(&s[16..28]); // frequency, pad, replay
        let blend = u32::from(u16::from_le_bytes([s[28], s[29]]));
        recs.extend_from_slice(&blend.to_le_bytes());
        recs.extend_from_slice(&s[32..62]); // bounds, radius, next variation
        recs.extend_from_slice(&(i as u16).to_le_bytes()); // its own alias
    }
    out.array(0x1c, n, &recs);

    // The animation lookup, by animation id: the first sequence of each.
    let ids: Vec<u16> = seqs
        .iter()
        .map(|s| u16::from_le_bytes([s[0], s[1]]))
        .collect();
    let mut lookup = vec![0xffffu16; ids.iter().max().map_or(0, |&m| m as usize + 1)];
    for (i, &id) in ids.iter().enumerate().rev() {
        lookup[id as usize] = i as u16;
    }
    let bytes: Vec<u8> = lookup.iter().flat_map(|l| l.to_le_bytes()).collect();
    out.array(0x24, lookup.len(), &bytes);
    // The playable lookup: an animation the model has plays itself, one it lacks the first of
    // its fallbacks it has, else the stand.
    let has = |id: u16| lookup.get(id as usize).is_some_and(|&l| l != 0xffff);
    let playable: Vec<u8> = (0..animation.fallbacks.len() as u16)
        .flat_map(|id| {
            let mut at = id;
            for _ in 0..16 {
                if has(at) {
                    break;
                }
                at = animation.fallbacks.get(at as usize).copied().unwrap_or(0);
            }
            u32::from(if has(at) { at } else { 0 }).to_le_bytes()
        })
        .collect();
    out.array(0x2c, animation.fallbacks.len(), &playable);

    // Bones: 88 bytes to 108, their translation, rotation and scale keyed.
    let (bones, n) = src.records(0x2c, 88)?;
    let bones_at = src.offset(0x2c)?;
    let mut recs = Vec::with_capacity(n * 108);
    for (i, b) in bones.as_chunks::<88>().0.iter().enumerate() {
        let at = bones_at + i * 88;
        recs.extend_from_slice(&b[0..12]); // key bone, flags, parent, submesh
        let identity = [0f32, 0.0, 0.0, 1.0];
        let rest: [&[u8]; 3] = [&floats(&[0.0; 3]), &floats(&identity), &floats(&[1.0; 3])];
        for (k, kind) in [Kind::Vec3, Kind::Quat, Kind::Vec3].into_iter().enumerate() {
            recs.extend_from_slice(&out.track(
                md20,
                at + 0x10 + k * 20,
                kind,
                Some(rest[k]),
                &bands,
            ));
        }
        recs.extend_from_slice(&b[76..88]); // pivot
    }
    out.array(0x34, n, &recs);

    // The one skin profile.
    let view = transcode_skin(&mut out, skin, &shows)?;
    let at = out.append(&view);
    out.put_u32(0x4c, 1);
    out.put_u32(0x50, at);

    // Colours: an RGB track and an alpha one. A sequence with no key holds the track's first.
    let (_, n) = src.records(0x48, 40)?;
    let mut recs = Vec::with_capacity(n * 0x38);
    for i in 0..n {
        let base = src.offset(0x48)? + i * 40;
        let rgb = out.track(md20, base, Kind::Vec3, None, &bands);
        let alpha = out.track(md20, base + 20, Kind::Fix16, None, &bands);
        recs.extend_from_slice(&rgb);
        recs.extend_from_slice(&alpha);
    }
    out.array(0x54, n, &recs);

    // Texture weights.
    let (_, n) = src.records(0x58, 20)?;
    let mut recs = Vec::with_capacity(n * TRACK);
    for i in 0..n {
        let base = src.offset(0x58)? + i * 20;
        recs.extend_from_slice(&out.track(md20, base, Kind::Fix16, None, &bands));
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

    // Texture transforms: a translation, a rotation (four floats in both) and a scaling, keyed.
    let (_, n) = src.records(0x60, 60)?;
    let mut recs = Vec::with_capacity(n * 0x54);
    for i in 0..n {
        let base = src.offset(0x60)? + i * 60;
        let identity = [0f32, 0.0, 0.0, 1.0];
        let rest: [&[u8]; 3] = [&floats(&[0.0; 3]), &floats(&identity), &floats(&[1.0; 3])];
        for (k, kind) in [Kind::Vec3, Kind::Quat32, Kind::Vec3]
            .into_iter()
            .enumerate()
        {
            recs.extend_from_slice(&out.track(md20, base + k * 20, kind, Some(rest[k]), &bands));
        }
    }
    out.array(0x74, n, &recs);
    out.copy(&src, 0x98, 0xac, 2)?; // texture transform lookup

    particles(&mut out, md20, &bands)?;
    ribbons(&mut out, md20, &bands)?;

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

/// A sequence's place on the 1.12.1 timeline, and where its keys are: the sequence whose arrays
/// it plays (itself, or the one an alias names) and the bytes their offsets count in (the model,
/// or its `.anim` file; none when that file is missing).
struct Band<'a> {
    start: u32,
    slot: usize,
    data: Option<&'a [u8]>,
}

/// Each sequence's band: one after another, a millisecond between them.
fn bands<'a>(md20: &'a [u8], seqs: &[[u8; 64]], files: &'a [(u16, u16, Vec<u8>)]) -> Vec<Band<'a>> {
    let flags = |i: usize| le32(&seqs[i], 12);
    let mut clock = 0u32;
    (0..seqs.len())
        .map(|i| {
            let mut slot = i;
            for _ in 0..seqs.len() {
                let alias = u16::from_le_bytes([seqs[slot][62], seqs[slot][63]]) as usize;
                if flags(slot) & 0x40 == 0 || alias == slot || alias >= seqs.len() {
                    break;
                }
                slot = alias;
            }
            let data = if flags(slot) & 0x20 != 0 {
                Some(md20)
            } else {
                let s = &seqs[slot];
                let (id, variation) = (
                    u16::from_le_bytes([s[0], s[1]]),
                    u16::from_le_bytes([s[2], s[3]]),
                );
                files
                    .iter()
                    .find(|f| (f.0, f.1) == (id, variation))
                    .map(|f| anim_body(&f.2))
            };
            let start = clock;
            clock = clock.saturating_add(le32(&seqs[i], 4)).saturating_add(1);
            Band { start, slot, data }
        })
        .collect()
}

/// An `.anim` file's key data: an `AFM2` chunk's body in a chunked one, else the whole file.
fn anim_body(file: &[u8]) -> &[u8] {
    if file.get(0..4) == Some(b"AFM2") {
        let size = u32_at(file, 4).unwrap_or(0) as usize;
        file.get(8..8 + size).unwrap_or(&[])
    } else {
        file
    }
}

/// What a track's values are.
#[derive(Clone, Copy)]
enum Kind {
    Vec3,
    /// Four `i16`, widened to four `f32`.
    Quat,
    /// An `i16` over 32767, as 1.12.1 keeps it.
    Fix16,
    /// Four floats, a texture transform's rotation.
    Quat32,
    F32,
    U16,
    U8,
    /// A particle's gravity packed as a vector (`int8` x and y, `int16` z: flag `0x800000`),
    /// brought to 1.12.1's pull down.
    Gravity,
}

impl Kind {
    /// A Legion value's size.
    fn size(self) -> usize {
        match self {
            Kind::Vec3 => 12,
            Kind::Quat => 8,
            Kind::Fix16 | Kind::U16 => 2,
            Kind::Quat32 => 16,
            Kind::F32 | Kind::Gravity => 4,
            Kind::U8 => 1,
        }
    }

    /// A Legion value as 1.12.1 keeps it.
    fn widen(self, v: &[u8]) -> Vec<u8> {
        match self {
            Kind::Quat => {
                let c: Vec<f32> = v
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|&b| {
                        let s = i32::from(i16::from_le_bytes(b));
                        (if s < 0 { s + 32768 } else { s - 32767 }) as f32 / 32767.0
                    })
                    .collect();
                floats(&c)
            }
            Kind::Gravity => floats(&[gravity(v)]),
            _ => v.to_vec(),
        }
    }
}

/// A packed gravity vector's pull down: its direction `(x/128, y/128, √(1 − x² − y²))` times
/// `z · 0.04238648`, the vector the particles fall along; 1.12.1's gravity pulls down the z axis,
/// so it keeps the vector's downward part.
fn gravity(v: &[u8]) -> f32 {
    let (x, y) = (f32::from(v[0] as i8) / 128.0, f32::from(v[1] as i8) / 128.0);
    let z = f32::from(i16::from_le_bytes([v[2], v[3]]));
    let up = (1.0 - x * x - y * y).max(0.0).sqrt();
    -(up * z * 0.042_386_48)
}

/// A particle's over-life ramp (an `FBlock`: times in `fixed16` of the particle's life, values)
/// at life fraction `t`, linear between keys; `None` with no keys.
fn over_life(
    md20: &[u8],
    at: usize,
    size: usize,
    t: f32,
    read: impl Fn(&[u8]) -> f32,
) -> Option<f32> {
    let (tn, to) = array_at(md20, at).ok()?;
    let (vn, vo) = array_at(md20, at + 8).ok()?;
    let keys: Vec<(f32, f32)> = (0..tn.min(vn))
        .map_while(|k| {
            let time = f32::from(u16_at(md20, to + k * 2)?) / 32767.0;
            Some((time, read(md20.get(vo + k * size..vo + (k + 1) * size)?)))
        })
        .collect();
    let first = *keys.first()?;
    if t <= first.0 {
        return Some(first.1);
    }
    for w in keys.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t <= t1 {
            let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return Some(v0 + (v1 - v0) * f);
        }
    }
    keys.last().map(|k| k.1)
}

fn f32_of(b: &[u8]) -> f32 {
    f32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// Particle emitters: Legion's 0x1ec-byte records to 1.12.1's 0x1f8. Their tracks are keyed as
/// the bones' are; the over-life ramps (`FBlock`s, any number of keys) are sampled at birth, at
/// the colour ramp's middle key and at death into 1.12.1's middle point and three keys. A
/// multi-texture emitter keeps its first texture, a "bone" emitter turns a plane, and Legion's
/// premultiplied blend (7) adds, as its glows mean. Emitters that spawn other models lose them.
fn particles(out: &mut Out, md20: &[u8], bands: &[Band]) -> Result<()> {
    const LEGION: usize = 0x1ec;
    let (n, base) = array_at(md20, 0x128)?;
    if n == 0 {
        return Ok(());
    }
    md20.get(base..base + n * LEGION)
        .ok_or(Error::Truncated("particles"))?;
    let mut recs = Vec::with_capacity(n * 0x1f8);
    for i in 0..n {
        let e = base + i * LEGION;
        let r = &md20[e..e + LEGION];
        let mut v = vec![0u8; 0x1f8];
        let flags = u32_at(r, 4).unwrap_or(0);
        v[0..4].copy_from_slice(&r[0..4]);
        // The flags 1.12.1 reads are its low sixteen bits.
        v[4..8].copy_from_slice(&(flags & 0xffff).to_le_bytes());
        v[8..0x16].copy_from_slice(&r[8..0x16]); // position, bone
        let texture = u16::from_le_bytes([r[0x16], r[0x17]]);
        let texture = if flags & 0x1000_0000 != 0 {
            texture & 0x1f
        } else {
            texture
        };
        v[0x16..0x18].copy_from_slice(&texture.to_le_bytes());
        let blend = match r[0x28] {
            b @ 0..=6 => b,
            _ => 4,
        };
        v[0x28] = blend;
        v[0x2a] = match r[0x29] {
            t @ 1..=3 => t,
            _ => 1,
        };
        v[0x2e..0x34].copy_from_slice(&r[0x2e..0x34]); // tile rotation, rows, columns
                                                       // Speed, its variation, the two spreads, gravity, lifespan, rate, the area's length and
                                                       // width, the z source; Legion keeps a variation float after the lifespan and the rate.
        let tracks = [0x34, 0x48, 0x5c, 0x70, 0x84, 0x98, 0xb0, 0xc8, 0xdc, 0xf0];
        for (k, &from) in tracks.iter().enumerate() {
            let kind = if k == 4 && flags & 0x80_0000 != 0 {
                Kind::Gravity
            } else {
                Kind::F32
            };
            let t = out.track(md20, e + from, kind, None, bands);
            v[0x34 + k * TRACK..0x34 + (k + 1) * TRACK].copy_from_slice(&t);
        }
        // The over-life ramps.
        let mid = {
            let (tn, to) = array_at(md20, e + 0x104)?;
            (tn >= 3)
                .then(|| u16_at(md20, to + 2))
                .flatten()
                .map_or(0.5, |t| (f32::from(t) / 32767.0).clamp(0.01, 0.99))
        };
        let at = [0.0, mid, 1.0];
        let channel = |c: usize| move |b: &[u8]| f32_of(&b[c * 4..]);
        let fix16 = |b: &[u8]| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32767.0;
        let cell = |b: &[u8]| f32::from(u16::from_le_bytes([b[0], b[1]]));
        v[0x14c..0x150].copy_from_slice(&mid.to_le_bytes());
        for (k, &t) in at.iter().enumerate() {
            let rgb =
                [0, 1, 2].map(|c| over_life(md20, e + 0x104, 12, t, channel(c)).unwrap_or(255.0));
            let alpha = over_life(md20, e + 0x114, 2, t, fix16).unwrap_or(1.0) * 255.0;
            let byte = |x: f32| x.round().clamp(0.0, 255.0) as u32;
            let bgra = byte(alpha) << 24 | byte(rgb[0]) << 16 | byte(rgb[1]) << 8 | byte(rgb[2]);
            v[0x150 + k * 4..0x154 + k * 4].copy_from_slice(&bgra.to_le_bytes());
            let scale = over_life(md20, e + 0x124, 8, t, f32_of).unwrap_or(1.0);
            v[0x15c + k * 4..0x160 + k * 4].copy_from_slice(&scale.to_le_bytes());
        }
        // The head's and the tail's flipbook cells, each segment from its key to the next.
        for (block, to) in [(0x13c, 0x168), (0x14c, 0x174)] {
            let c = at.map(|t| over_life(md20, e + block, 2, t, cell).unwrap_or(0.0) as u16);
            let mut ramp = vec![c[0], c[1]];
            if to == 0x168 {
                ramp.push(1); // the head's repeat count, after each pair
            }
            ramp.extend([c[1], c[2]]);
            if to == 0x168 {
                ramp.push(1);
            }
            let bytes: Vec<u8> = ramp.iter().flat_map(|x| x.to_le_bytes()).collect();
            v[to..to + bytes.len()].copy_from_slice(&bytes);
        }
        // A tail where the tail cells are keyed: 2 is head and tail, 0 the head alone.
        let tailed = array_at(md20, e + 0x14c).is_ok_and(|(k, _)| k > 0);
        v[0x2c] = if tailed { 2 } else { 0 };
        v[0x17c..0x198].copy_from_slice(&r[0x15c..0x178]); // tail, twinkle, burst, drag
        v[0x198..0x19c].copy_from_slice(&r[0x180..0x184]); // spin
        v[0x19c..0x1d4].copy_from_slice(&r[0x188..0x1c0]); // tumble, wind, follow
        let (sn, so) = array_at(md20, e + 0x1c0)?;
        let points = md20.get(so..so + sn * 12).unwrap_or(&[]);
        let (sn, so) = out.put_array(points.len() / 12, points);
        v[0x1d4..0x1d8].copy_from_slice(&sn.to_le_bytes());
        v[0x1d8..0x1dc].copy_from_slice(&so.to_le_bytes());
        let enabled = out.track(md20, e + 0x1c8, Kind::U8, None, bands);
        v[0x1dc..0x1f8].copy_from_slice(&enabled);
        recs.extend_from_slice(&v);
    }
    out.array(0x13c, n, &recs);
    Ok(())
}

/// Ribbon emitters: Legion's 0xb0-byte records to 1.12.1's 0xdc, their tracks keyed.
fn ribbons(out: &mut Out, md20: &[u8], bands: &[Band]) -> Result<()> {
    const LEGION: usize = 0xb0;
    let (n, base) = array_at(md20, 0x120)?;
    if n == 0 {
        return Ok(());
    }
    md20.get(base..base + n * LEGION)
        .ok_or(Error::Truncated("ribbons"))?;
    let mut recs = Vec::with_capacity(n * 0xdc);
    for i in 0..n {
        let e = base + i * LEGION;
        let r = &md20[e..e + LEGION];
        let mut v = vec![0u8; 0xdc];
        v[0..0x14].copy_from_slice(&r[0..0x14]); // id, bone, position
                                                 // The texture and material indices, copied out.
        for at in [0x14, 0x1c] {
            let (k, o) = array_at(md20, e + at)?;
            let list = md20.get(o..o + k * 2).unwrap_or(&[]);
            let (k, o) = out.put_array(list.len() / 2, list);
            v[at..at + 4].copy_from_slice(&k.to_le_bytes());
            v[at + 4..at + 8].copy_from_slice(&o.to_le_bytes());
        }
        for (from, to, kind) in [
            (0x24, 0x24, Kind::Vec3),
            (0x38, 0x40, Kind::Fix16),
            (0x4c, 0x5c, Kind::F32),
            (0x60, 0x78, Kind::F32),
        ] {
            let t = out.track(md20, e + from, kind, None, bands);
            v[to..to + TRACK].copy_from_slice(&t);
        }
        v[0x94..0xa4].copy_from_slice(&r[0x74..0x84]); // edges, lifetime, gravity, rows, columns
        let slot = out.track(md20, e + 0x84, Kind::U16, None, bands);
        v[0xa4..0xc0].copy_from_slice(&slot);
        let shown = out.track(md20, e + 0x98, Kind::U8, None, bands);
        v[0xc0..0xdc].copy_from_slice(&shown);
        recs.extend_from_slice(&v);
    }
    out.array(0x134, n, &recs);
    Ok(())
}

fn floats(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
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

    /// The Legion track at `at` as a 1.12.1 one: each sequence's keys on its band of the
    /// timeline, and its window of them. A sequence with none takes `rest`, else the track's first
    /// key; a track with no key anywhere stays empty. A global sequence's track is its one array.
    fn track(
        &mut self,
        md20: &[u8],
        at: usize,
        kind: Kind,
        rest: Option<&[u8]>,
        bands: &[Band],
    ) -> [u8; TRACK] {
        let (Some(interp), Some(gseq)) = (u16_at(md20, at), u16_at(md20, at + 2)) else {
            return empty_track();
        };
        let (Ok((tn, to)), Ok((vn, vo))) = (array_at(md20, at + 4), array_at(md20, at + 12)) else {
            return empty_track();
        };
        // A spline key is its value and two tangents; only the value is kept.
        let stride = kind.size() * if interp >= 2 { 3 } else { 1 };
        // Sequence `slot`'s keys, its arrays' headers in the model, their data in `data`.
        let keys = |data: &[u8], slot: usize| -> Vec<(u32, Vec<u8>)> {
            if slot >= tn.min(vn) {
                return Vec::new();
            }
            let (Ok((tc, toff)), Ok((vc, voff))) =
                (array_at(md20, to + slot * 8), array_at(md20, vo + slot * 8))
            else {
                return Vec::new();
            };
            (0..tc.min(vc))
                .map_while(|k| {
                    let t = u32_at(data, toff + k * 4)?;
                    let v = data.get(voff + k * stride..voff + k * stride + kind.size())?;
                    Some((t, kind.widen(v)))
                })
                .collect()
        };
        let mut t = empty_track();
        t[0..2].copy_from_slice(&interp.min(1).to_le_bytes());
        t[2..4].copy_from_slice(&gseq.to_le_bytes());
        let (all, ranges): (Vec<(u32, Vec<u8>)>, Vec<u8>) = if gseq != 0xffff {
            (keys(md20, 0), Vec::new())
        } else {
            let per: Vec<Vec<(u32, Vec<u8>)>> = bands
                .iter()
                .map(|b| b.data.map_or_else(Vec::new, |d| keys(d, b.slot)))
                .collect();
            let Some(first) = per.iter().flatten().next().map(|(_, v)| v.clone()) else {
                return empty_track();
            };
            let rest = rest.map_or(first, <[u8]>::to_vec);
            let mut all = Vec::new();
            let mut ranges = Vec::with_capacity(per.len() * 8);
            for (band, mut ks) in bands.iter().zip(per) {
                if ks.is_empty() {
                    ks.push((0, rest.clone()));
                }
                let lo = all.len() as u32;
                ranges.extend_from_slice(&lo.to_le_bytes());
                ranges.extend_from_slice(&(lo + ks.len() as u32 - 1).to_le_bytes());
                all.extend(
                    ks.into_iter()
                        .map(|(at, v)| (band.start.saturating_add(at), v)),
                );
            }
            (all, ranges)
        };
        if all.is_empty() {
            return t;
        }
        let times: Vec<u8> = all.iter().flat_map(|(at, _)| at.to_le_bytes()).collect();
        let values: Vec<u8> = all.iter().flat_map(|(_, v)| v.iter().copied()).collect();
        let n = all.len() as u32;
        let (rn, ro) = self.put_array(ranges.len() / 8, &ranges);
        let times = self.append(&times);
        let values = self.append(&values);
        for (at, v) in [
            (4, rn),
            (8, ro),
            (12, n),
            (16, times),
            (20, n),
            (24, values),
        ] {
            t[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::CreatureDisplay;
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
        seq[12..16].copy_from_slice(&0x20u32.to_le_bytes()); // its keys in the model
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
        let file = to_vanilla(&md20, &skin, &names, |id| id != 102, &Animation::default()).unwrap();
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
        assert_eq!(m.texture_transform_lookup, [0]);
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
        let file = to_vanilla(&md20, &skin, &[None], |_| true, &Animation::default()).unwrap();
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

    /// Three sequences: a stand keyed in the model, an alias of it, and a spell keyed in its
    /// `.anim` file; one bone turns in each.
    #[test]
    fn keys_go_on_each_sequences_band() {
        let (md20, skin) = legion();
        let mut m = Out { b: md20 };
        let seq = |id: u16, duration: u32, flags: u32, alias: u16| {
            let mut s = vec![0u8; 64];
            s[0..2].copy_from_slice(&id.to_le_bytes());
            s[4..8].copy_from_slice(&duration.to_le_bytes());
            s[12..16].copy_from_slice(&flags.to_le_bytes());
            s[60..62].copy_from_slice(&(-1i16).to_le_bytes());
            s[62..64].copy_from_slice(&alias.to_le_bytes());
            s
        };
        let seqs = [
            seq(0, 100, 0x20, 0),
            seq(108, 100, 0x60, 0),
            seq(60, 200, 0x1, 2),
        ];
        m.array(0x1c, 3, &seqs.concat());
        // Compressed quaternions: -1 is 1.0, 32767 is 0.0, -16385 about one half.
        let q = |w: i16| [32767i16, 32767, 32767, w].map(i16::to_le_bytes).concat();
        let times = m.append(&[0u32, 100].map(u32::to_le_bytes).concat());
        let values = m.append(&[q(-1), q(-16385)].concat());
        let pair = |n: u32, at: u32| [n.to_le_bytes(), at.to_le_bytes()].concat();
        // The spell's arrays count in its .anim file: a time at 0, a value at 4.
        let outer_t = m.append(&[pair(2, times), pair(0, 0), pair(1, 0)].concat());
        let outer_v = m.append(&[pair(2, values), pair(0, 0), pair(1, 4)].concat());
        let mut bone = vec![0u8; 88];
        bone[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        bone[8..10].copy_from_slice(&(-1i16).to_le_bytes());
        let track = 0x24;
        bone[track..track + 2].copy_from_slice(&1u16.to_le_bytes());
        bone[track + 2..track + 4].copy_from_slice(&0xffffu16.to_le_bytes());
        bone[track + 4..track + 12].copy_from_slice(&pair(3, outer_t));
        bone[track + 12..track + 20].copy_from_slice(&pair(3, outer_v));
        m.array(0x2c, 1, &bone);
        let anim = [50u32.to_le_bytes().to_vec(), q(-16385)].concat();
        let mut fallbacks = vec![0u16; 200];
        fallbacks[5] = 108;
        let animation = Animation {
            files: &[(60, 0, anim)],
            fallbacks: &fallbacks,
        };
        let file = to_vanilla(&m.b, &skin, &[None], |_| true, &animation).unwrap();

        let clips = benilla_formats::parse_m2_animations(&file);
        let turns = |id: u16| {
            let c = clips.iter().find(|c| c.anim_id == id).unwrap();
            c.bones[0]
                .rotation
                .iter()
                .map(|k| k.0)
                .collect::<Vec<f32>>()
        };
        assert_eq!(turns(0), [0.0, 0.1]);
        // The alias plays its sequence's keys on its own band.
        assert_eq!(turns(108), [0.0, 0.1]);
        // The .anim file's key, 50 ms into the spell.
        assert_eq!(turns(60), [0.05]);
        let spell = clips.iter().find(|c| c.anim_id == 60).unwrap();
        assert!(!spell.looping);
        assert_eq!((spell.start_ms, spell.end_ms), (202, 402));
        let w = spell.bones[0].rotation[0].1;
        assert!(
            (w.iter().map(|c| c * c).sum::<f32>().sqrt() - 0.5).abs() < 1e-3,
            "{w:?}"
        );

        // Direct by id, and the fallbacks for the rest.
        let lookup = benilla_formats::parse_m2_animation_lookup(&file).unwrap();
        assert_eq!((lookup[0], lookup[60], lookup[108]), (0, 2, 1));
        assert_eq!(lookup[5], 0xffff);
        let playable = benilla_formats::parse_m2_playable_animation_lookup(&file).unwrap();
        assert_eq!(playable.len(), 200);
        let to = |id: usize| playable[id].resolved_id;
        assert_eq!((to(0), to(5), to(60), to(7)), (0, 108, 60, 0));
    }

    /// An `FBlock` of three keys, its times at birth, `mid` and death.
    fn fblock(m: &mut Out, mid: u16, values: &[u8]) -> [u8; 16] {
        let times = m.append(&[0u16, mid, 32767].map(u16::to_le_bytes).concat());
        let values = m.append(values);
        let mut b = [0u8; 16];
        for (at, v) in [(0, 3), (4, times), (8, 3), (12, values)] {
            b[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        b
    }

    /// A model with one particle emitter (its over-life ramps keyed, Legion's blend 7, a packed
    /// gravity), one ribbon and one texture transform, read back by the client's parsers.
    #[test]
    fn emitters_and_texture_transforms_read_as_the_clients() {
        let (md20, skin) = legion();
        let mut m = Out { b: md20 };
        let mut p = vec![0u8; 0x1ec];
        p[0..4].copy_from_slice(&(-1i32).to_le_bytes());
        // Unlit, a packed gravity, a multi-texture.
        p[4..8].copy_from_slice(&(0x1 | 0x80_0000 | 0x1000_0000u32).to_le_bytes());
        // Textures 0 and 1 packed five bits each: the first is kept.
        p[0x16..0x18].copy_from_slice(&(1u16 << 5).to_le_bytes());
        p[0x28] = 7;
        p[0x29] = 2;
        p[0x30..0x34].copy_from_slice(&[1, 0, 1, 0]);
        let rgb = [[255f32, 0.0, 0.0], [0.0, 255.0, 0.0], [0.0, 0.0, 255.0]];
        let color = fblock(&mut m, 8191, &floats(&rgb.concat()));
        p[0x104..0x114].copy_from_slice(&color);
        let alpha = fblock(
            &mut m,
            8191,
            &[32767i16, 16383, 0].map(i16::to_le_bytes).concat(),
        );
        p[0x114..0x124].copy_from_slice(&alpha);
        let scale = fblock(&mut m, 8191, &floats(&[1.0, 1.0, 2.0, 2.0, 0.5, 0.5]));
        p[0x124..0x134].copy_from_slice(&scale);
        p[0x174..0x178].copy_from_slice(&0.25f32.to_le_bytes()); // drag
                                                                 // Gravity: straight down the packed vector's z, one key in the stand.
        let pair = |n: u32, at: u32| [n.to_le_bytes(), at.to_le_bytes()].concat();
        let time = m.append(&0u32.to_le_bytes());
        let value = m.append(&[0u8, 0, 100, 0]);
        let ot = m.append(&pair(1, time));
        let ov = m.append(&pair(1, value));
        p[0x84 + 2..0x84 + 4].copy_from_slice(&0xffffu16.to_le_bytes());
        p[0x84 + 4..0x84 + 12].copy_from_slice(&pair(1, ot));
        p[0x84 + 12..0x84 + 20].copy_from_slice(&pair(1, ov));
        m.array(0x128, 1, &p);

        let mut r = vec![0u8; 0xb0];
        r[4..8].copy_from_slice(&0u32.to_le_bytes());
        let tex = m.append(&0u16.to_le_bytes());
        r[0x14..0x1c].copy_from_slice(&pair(1, tex));
        let mat = m.append(&0u16.to_le_bytes());
        r[0x1c..0x24].copy_from_slice(&pair(1, mat));
        r[0x74..0x78].copy_from_slice(&30f32.to_le_bytes());
        r[0x78..0x7c].copy_from_slice(&0.5f32.to_le_bytes());
        m.array(0x120, 1, &r);

        let mut t = vec![0u8; 60];
        for k in 0..3 {
            t[k * 20 + 2..k * 20 + 4].copy_from_slice(&0xffffu16.to_le_bytes());
        }
        m.array(0x60, 1, &t);
        m.array(0x98, 1, &[0, 0]);

        let names = [Some(virtual_path(7))];
        let file = to_vanilla(&m.b, &skin, &names, |_| true, &Animation::default()).unwrap();

        let ps = benilla_formats::parse_m2_particle_emitters(&file).unwrap();
        assert_eq!(ps.len(), 1);
        let p = &ps[0];
        assert_eq!(p.blend, benilla_formats::ParticleBlend::Add);
        assert_eq!(p.shape, benilla_formats::ParticleShape::Sphere);
        assert!(!p.lit);
        assert_eq!(p.texture.as_deref(), Some("Legion\\7.blp"));
        assert_eq!(p.drag, 0.25);
        let life = &p.over_life;
        assert!((life.mid - 0.25).abs() < 1e-3, "{}", life.mid);
        assert_eq!(life.color[0], [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(life.color[1][1], 1.0);
        assert!((life.color[1][3] - 0.5).abs() < 0.01);
        assert_eq!(life.color[2], [0.0, 0.0, 1.0, 0.0]);
        assert_eq!(life.scale, [1.0, 2.0, 0.5]);
        // The packed gravity: z 100 straight up the vector, a pull down of about 4.24.
        assert!((gravity(&[0, 0, 100, 0]) + 4.238648).abs() < 1e-4);

        let rs = benilla_formats::parse_m2_ribbon_emitters(&file).unwrap();
        assert_eq!(rs.len(), 1);
        assert_eq!(rs[0].texture.as_deref(), Some("Legion\\7.blp"));

        let f = benilla_m2::parse_m2(&mut Cursor::new(&file[..])).unwrap();
        assert_eq!(f.model().texture_transforms.len(), 1);
        assert_eq!(f.model().texture_transform_lookup, [0]);
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
        let files = texture_files(
            &model,
            &[5],
            |k| display.texture_for(k),
            |p| (p == "a.blp").then_some(6),
        );
        assert_eq!(files, [Some(5), Some(6), Some(77), None]);
    }
}
