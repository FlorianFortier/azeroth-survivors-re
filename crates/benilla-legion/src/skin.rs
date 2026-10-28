//! A `.skin` file, a skin profile out of its model since Wrath: `SKIN`, then the arrays 1.12.1's
//! embedded views hold (vertex indices into the model, triangles into those, bone properties,
//! sections, batches), with offsets from the file's start. A section is 48 bytes (32 in 1.12.1);
//! from Legion its `level` word carries the high bits of its triangles' start, for profiles past
//! 65535 indices (a profile's vertices stay under 65536).

use crate::{array_at, records, u16_at, Error, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    /// The geoset id, `group * 100 + variant`.
    pub id: u16,
    pub vertex_start: u32,
    pub vertex_count: u16,
    pub index_start: u32,
    pub index_count: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Batch {
    pub flags: u8,
    pub priority: i8,
    pub shader_id: u16,
    pub section: u16,
    pub color: u16,
    pub material: u16,
    pub texture_count: u16,
    pub texture_combo: u16,
    pub uv_combo: u16,
    pub weight_combo: u16,
    pub transform_combo: u16,
}

#[derive(Debug)]
pub struct Skin {
    /// The model vertex each skin vertex is.
    pub vertices: Vec<u16>,
    /// Triangles, three skin vertices each.
    pub indices: Vec<u16>,
    pub sections: Vec<Section>,
    pub batches: Vec<Batch>,
}

impl Skin {
    /// A section's triangles as model vertex indices.
    pub fn triangles(&self, section: &Section) -> impl Iterator<Item = [usize; 3]> + '_ {
        let start = section.index_start as usize;
        let end = (start + section.index_count as usize).min(self.indices.len());
        self.indices[start.min(end)..end]
            .as_chunks::<3>()
            .0
            .iter()
            .filter_map(|t| {
                let v = |i: u16| self.vertices.get(i as usize).map(|&m| m as usize);
                Some([v(t[0])?, v(t[1])?, v(t[2])?])
            })
    }
}

pub fn parse(b: &[u8]) -> Result<Skin> {
    if b.get(0..4) != Some(b"SKIN") {
        return Err(Error::Format("not SKIN".into()));
    }
    let u16s = |at: usize, what| records(b, array_at(b, at)?, 2, what, |r| u16_at(r, 0));
    let vertices = u16s(4, "skin vertices")?;
    let indices = u16s(12, "skin indices")?;
    let sections = records(b, array_at(b, 28)?, 48, "sections", |s| {
        let level = u32::from(u16_at(s, 2)?);
        Some(Section {
            id: u16_at(s, 0)?,
            vertex_start: u32::from(u16_at(s, 4)?),
            vertex_count: u16_at(s, 6)?,
            index_start: u32::from(u16_at(s, 8)?) + (level << 16),
            index_count: u16_at(s, 10)?,
        })
    })?;
    let batches = records(b, array_at(b, 36)?, 24, "batches", |u| {
        Some(Batch {
            flags: u[0],
            priority: u[1] as i8,
            shader_id: u16_at(u, 2)?,
            section: u16_at(u, 4)?,
            color: u16_at(u, 8)?,
            material: u16_at(u, 10)?,
            texture_count: u16_at(u, 14)?,
            texture_combo: u16_at(u, 16)?,
            uv_combo: u16_at(u, 18)?,
            weight_combo: u16_at(u, 20)?,
            transform_combo: u16_at(u, 22)?,
        })
    })?;
    Ok(Skin {
        vertices,
        indices,
        sections,
        batches,
    })
}
