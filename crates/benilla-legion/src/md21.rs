//! The chunked `.m2` Legion writes: chunks of a 4-byte id, a little-endian size and the body.
//! `MD21` holds the MD20 model; the others are tables of file data ids.

use crate::{u32_at, Error, Result};

/// A chunked model's MD20 bytes and the files it names.
#[derive(Debug, Default)]
pub struct Md21 {
    pub md20: Vec<u8>,
    /// Skin profiles, `<name>00.skin` first, then the level-of-detail ones.
    pub skins: Vec<u32>,
    /// One per texture record, 0 for a texture the model does not name (a replaceable one).
    pub textures: Vec<u32>,
    /// `(animation id, sub-animation id, file data id)` of the sequences kept in `.anim` files.
    pub anims: Vec<(u16, u16, u32)>,
    /// Every other chunk's id, for a probe.
    pub other: Vec<[u8; 4]>,
}

pub fn split(file: &[u8]) -> Result<Md21> {
    if file.get(0..4) == Some(b"MD20") {
        // A pre-Legion model: no chunks.
        return Ok(Md21 {
            md20: file.to_vec(),
            ..Md21::default()
        });
    }
    let mut out = Md21::default();
    let mut at = 0;
    while at + 8 <= file.len() {
        let id: [u8; 4] = file[at..at + 4].try_into().unwrap();
        let size = u32_at(file, at + 4).ok_or(Error::Truncated("chunk"))? as usize;
        let body = file
            .get(at + 8..at + 8 + size)
            .ok_or(Error::Truncated("chunk body"))?;
        let ids = || {
            body.as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
        };
        match &id {
            b"MD21" => out.md20 = body.to_vec(),
            b"SFID" => out.skins = ids().collect(),
            b"TXID" => out.textures = ids().collect(),
            b"AFID" => {
                out.anims = body
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|c| {
                        (
                            u16::from_le_bytes([c[0], c[1]]),
                            u16::from_le_bytes([c[2], c[3]]),
                            u32::from_le_bytes([c[4], c[5], c[6], c[7]]),
                        )
                    })
                    .collect()
            }
            _ => out.other.push(id),
        }
        at += 8 + size;
    }
    if out.md20.get(0..4) != Some(b"MD20") {
        return Err(Error::Format("no MD21 chunk holding an MD20 model".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_name_the_model_and_its_files() {
        let mut f = Vec::new();
        let mut chunk = |id: &[u8; 4], body: &[u8]| {
            f.extend_from_slice(id);
            f.extend_from_slice(&(body.len() as u32).to_le_bytes());
            f.extend_from_slice(body);
        };
        chunk(b"MD21", b"MD20\x12\x01\x00\x00");
        chunk(b"SFID", &[7, 0, 0, 0, 8, 0, 0, 0]);
        chunk(b"TXID", &[0, 0, 0, 0, 9, 0, 0, 0]);
        chunk(b"PFID", &[1, 0, 0, 0]);
        let m = split(&f).unwrap();
        assert_eq!(&m.md20[..4], b"MD20");
        assert_eq!(m.skins, [7, 8]);
        assert_eq!(m.textures, [0, 9]);
        assert_eq!(m.other, [*b"PFID"]);
    }
}
