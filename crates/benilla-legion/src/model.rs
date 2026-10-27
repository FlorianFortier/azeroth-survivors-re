//! The MD20 model inside an MD21 chunk, version 272 to 274: what a bind-pose draw needs.
//!
//! The header walk is Wrath's: version 264 dropped the playable animation lookup (`+0x2c` in
//! 1.12.1), put a skin profile count where the embedded views were (`+0x44`) and dropped the
//! texture flipbooks, so every later field sits where 1.12.1's reader already expects it for a
//! version above 263. Records holding tracks shrink: an `M2Track` is 20 bytes (interpolation,
//! global sequence, then the timestamps and the values, each an array of per-sequence arrays),
//! so a bone is 88 bytes (108 in 1.12.1).

use crate::{array_at, f32_at, records, u16_at, u32_at, Error, Result};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub position: [f32; 3],
    pub bone_weights: [u8; 4],
    pub bone_indices: [u8; 4],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub uv2: [f32; 2],
}

/// A texture record: its type (0 named by the model, 1 a character's skin, 11 to 13 a creature's
/// replaceable skins…), its wrap flags, and the path a type 0 names. A model converted later in
/// Legion names its files in `TXID` instead, and leaves the path empty.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    pub kind: u32,
    pub flags: u32,
    pub filename: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    pub flags: u16,
    /// 0 opaque, 1 alpha key, 2 alpha blend, 3+ additive and modulate forms.
    pub blend: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bone {
    pub key_bone: i32,
    pub flags: u32,
    pub parent: i16,
    pub pivot: [f32; 3],
}

#[derive(Debug)]
pub struct Model {
    pub version: u32,
    pub name: String,
    pub sequences: usize,
    pub skin_profiles: u32,
    pub vertices: Vec<Vertex>,
    pub textures: Vec<Texture>,
    pub materials: Vec<Material>,
    pub bones: Vec<Bone>,
    pub texture_lookup: Vec<u16>,
    pub bounds: ([f32; 3], [f32; 3]),
}

impl Model {
    /// The texture record a batch's `texture_combo_index` names.
    pub fn texture_of(&self, combo: u16) -> Option<usize> {
        self.texture_lookup
            .get(combo as usize)
            .map(|&t| t as usize)
            .filter(|&t| t < self.textures.len())
    }
}

fn c3(r: &[u8], at: usize) -> Option<[f32; 3]> {
    Some([f32_at(r, at)?, f32_at(r, at + 4)?, f32_at(r, at + 8)?])
}

pub fn parse(md20: &[u8]) -> Result<Model> {
    if md20.get(0..4) != Some(b"MD20") {
        return Err(Error::Format("not MD20".into()));
    }
    let version = u32_at(md20, 4).ok_or(Error::Truncated("version"))?;
    if !(264..=274).contains(&version) {
        return Err(Error::Format(format!("MD20 version {version}")));
    }
    let name = {
        let (n, o) = array_at(md20, 0x08)?;
        let raw = md20.get(o..o + n).unwrap_or(&[]);
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        String::from_utf8_lossy(&raw[..end]).into_owned()
    };
    let sequences = array_at(md20, 0x1c)?.0;
    let skin_profiles = u32_at(md20, 0x44).ok_or(Error::Truncated("skin count"))?;
    let vertices = records(md20, array_at(md20, 0x3c)?, 48, "vertices", |v| {
        Some(Vertex {
            position: c3(v, 0)?,
            bone_weights: [v[12], v[13], v[14], v[15]],
            bone_indices: [v[16], v[17], v[18], v[19]],
            normal: c3(v, 20)?,
            uv: [f32_at(v, 32)?, f32_at(v, 36)?],
            uv2: [f32_at(v, 40)?, f32_at(v, 44)?],
        })
    })?;
    let textures = records(md20, array_at(md20, 0x50)?, 16, "textures", |t| {
        let (n, o) = (u32_at(t, 8)? as usize, u32_at(t, 12)? as usize);
        let raw = md20.get(o..o + n).unwrap_or(&[]);
        let end = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
        Some(Texture {
            kind: u32_at(t, 0)?,
            flags: u32_at(t, 4)?,
            filename: String::from_utf8_lossy(&raw[..end]).into_owned(),
        })
    })?;
    let materials = records(md20, array_at(md20, 0x70)?, 4, "materials", |m| {
        Some(Material {
            flags: u16_at(m, 0)?,
            blend: u16_at(m, 2)?,
        })
    })?;
    let bones = records(md20, array_at(md20, 0x2c)?, 88, "bones", |b| {
        Some(Bone {
            key_bone: u32_at(b, 0)? as i32,
            flags: u32_at(b, 4)?,
            parent: u16_at(b, 8)? as i16,
            pivot: c3(b, 76)?,
        })
    })?;
    let texture_lookup = records(md20, array_at(md20, 0x80)?, 2, "texture lookup", |l| {
        u16_at(l, 0)
    })?;
    let bounds = (
        c3(md20, 0xa0).ok_or(Error::Truncated("bounds"))?,
        c3(md20, 0xac).ok_or(Error::Truncated("bounds"))?,
    );
    Ok(Model {
        version,
        name,
        sequences,
        skin_profiles,
        vertices,
        textures,
        materials,
        bones,
        texture_lookup,
        bounds,
    })
}
