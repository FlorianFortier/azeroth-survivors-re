//! A body texture layout that is not 1.12.1's, brought to 1.12.1's 256² atlas: the Pandaren's
//! (`CharComponentTextureLayouts` 2, 1024×512).
//!
//! Its left half is the old layout at twice 1.12.1's size (`CharComponentTextureSections` 0 to 7,
//! the arms, hands, torso, legs and feet, at the same places as layout 1's), so it halves onto
//! 1.12.1's tiles as the other races' textures do. Its right half is the face: one 512² square
//! (sections 9 and 10 both), where 1.12.1 has two strips under the arms, face upper
//! ([`FACE_UPPER`], 128×32) and face lower (128×64), one 128×96 block ([`FACE_BLOCK`]). The square
//! is drawn into that block: the skin atlas ([`skin_atlas`]), each face texture as the two strips'
//! tiles ([`face_tile`]), and the body model's skin coordinates moved with them
//! ([`remap_body_uvs`]). The face is squeezed from 1:1 to 4:3 there and drawn at a quarter of
//! its width; 1.12.1's own faces are drawn at that size.

use crate::skin::Skin;
use crate::{Error, Result};

/// The layout: 1024×512, the face square at its right.
const WIDTH: usize = 1024;
const HEIGHT: usize = 512;
const FACE_SQUARE: (usize, usize, usize, usize) = (512, 0, 512, 512);
/// 1.12.1's atlas, and its two face strips as one block: `(x, y, w, h)` in its 256².
const ATLAS: usize = 256;
pub const FACE_BLOCK: (usize, usize, usize, usize) = (0, 160, 128, 96);
/// The upper strip's height in the block (1.12.1's tile G8, 128×32); the lower is the rest (G9).
pub const FACE_UPPER: usize = 32;

/// A decoded image, RGBA8.
struct Image {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

fn decode(blp: &[u8]) -> Result<Image> {
    let d = benilla_blp::decode(blp).map_err(|e| Error::Format(format!("BLP: {e}")))?;
    let m = d
        .mips
        .into_iter()
        .next()
        .ok_or_else(|| Error::Format("BLP: no level".into()))?;
    Ok(Image {
        w: m.width as usize,
        h: m.height as usize,
        px: m.rgba,
    })
}

/// The rectangle `(x, y, w, h)` of `src` resampled to `dw`×`dh`, each output texel the average
/// of the source area it covers (a box filter: the downscales here are 2× to 5⅓×).
fn resample(
    src: &Image,
    (x, y, w, h): (usize, usize, usize, usize),
    dw: usize,
    dh: usize,
) -> Image {
    let mut px = vec![0u8; dw * dh * 4];
    let (sx, sy) = (w as f32 / dw as f32, h as f32 / dh as f32);
    for oy in 0..dh {
        let (y0, y1) = (oy as f32 * sy, (oy + 1) as f32 * sy);
        for ox in 0..dw {
            let (x0, x1) = (ox as f32 * sx, (ox + 1) as f32 * sx);
            let mut acc = [0f32; 4];
            let mut weight = 0f32;
            for iy in y0.floor() as usize..(y1.ceil() as usize).min(h) {
                let wy = (y1.min(iy as f32 + 1.0) - y0.max(iy as f32)).max(0.0);
                for ix in x0.floor() as usize..(x1.ceil() as usize).min(w) {
                    let wx = (x1.min(ix as f32 + 1.0) - x0.max(ix as f32)).max(0.0);
                    let k = wx * wy;
                    let at = ((y + iy).min(src.h - 1) * src.w + (x + ix).min(src.w - 1)) * 4;
                    for (a, &v) in acc.iter_mut().zip(&src.px[at..at + 4]) {
                        *a += f32::from(v) * k;
                    }
                    weight += k;
                }
            }
            let at = (oy * dw + ox) * 4;
            for (p, a) in px[at..at + 4].iter_mut().zip(acc) {
                *p = (a / weight.max(1e-6)).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    Image { w: dw, h: dh, px }
}

/// `img` drawn into `dst` at `(x, y)`.
fn paste(dst: &mut Image, img: &Image, x: usize, y: usize) {
    for row in 0..img.h {
        let from = row * img.w * 4;
        let to = ((y + row) * dst.w + x) * 4;
        dst.px[to..to + img.w * 4].copy_from_slice(&img.px[from..from + img.w * 4]);
    }
}

/// An image as a BLP2 1.12.1 reads: uncompressed BGRA (compression 3), 8-bit alpha, its whole
/// mip chain down to 1×1, each level the box average of the one above.
fn encode(img: &Image) -> Vec<u8> {
    const HEADER: usize = 148 + 1024;
    let mut levels = vec![Image {
        w: img.w,
        h: img.h,
        px: img.px.clone(),
    }];
    while levels.len() < 16 {
        let last = levels.last().unwrap();
        if last.w == 1 && last.h == 1 {
            break;
        }
        let next = resample(
            last,
            (0, 0, last.w, last.h),
            (last.w / 2).max(1),
            (last.h / 2).max(1),
        );
        levels.push(next);
    }
    let mut out = vec![0u8; HEADER];
    out[0..4].copy_from_slice(b"BLP2");
    out[4..8].copy_from_slice(&1u32.to_le_bytes());
    out[8] = 3;
    out[9] = 8;
    out[10] = 0;
    out[11] = 1;
    out[12..16].copy_from_slice(&(img.w as u32).to_le_bytes());
    out[16..20].copy_from_slice(&(img.h as u32).to_le_bytes());
    for (i, l) in levels.iter().enumerate() {
        let at = out.len() as u32;
        let size = (l.w * l.h * 4) as u32;
        out[20 + i * 4..24 + i * 4].copy_from_slice(&at.to_le_bytes());
        out[84 + i * 4..88 + i * 4].copy_from_slice(&size.to_le_bytes());
        for p in l.px.as_chunks::<4>().0 {
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

/// A skin texture of the layout as 1.12.1's 256² atlas: the body halved where it is, the face
/// square into the face block.
pub fn skin_atlas(blp: &[u8]) -> Result<Vec<u8>> {
    let src = decode(blp)?;
    // The layout at whatever size the file is (its own mips aside).
    let k = src.w as f32 / WIDTH as f32;
    let sc = |r: (usize, usize, usize, usize)| {
        let f = |v: usize| (v as f32 * k) as usize;
        (f(r.0), f(r.1), f(r.2), f(r.3))
    };
    if (src.h as f32 / HEIGHT as f32 - k).abs() > 0.01 {
        return Err(Error::Format(format!(
            "skin {}×{} is not the 2:1 layout",
            src.w, src.h
        )));
    }
    let mut atlas = resample(&src, sc((0, 0, 512, 512)), ATLAS, ATLAS);
    let face = resample(&src, sc(FACE_SQUARE), FACE_BLOCK.2, FACE_BLOCK.3);
    paste(&mut atlas, &face, FACE_BLOCK.0, FACE_BLOCK.1);
    Ok(encode(&atlas))
}

/// A face texture of the layout (the face square) as one of 1.12.1's two face tiles: the upper
/// strip (128×32) or the lower (128×64) of the square drawn into the face block.
pub fn face_tile(blp: &[u8], upper: bool) -> Result<Vec<u8>> {
    let src = decode(blp)?;
    let block = resample(&src, (0, 0, src.w, src.h), FACE_BLOCK.2, FACE_BLOCK.3);
    let (from, h) = if upper {
        (0, FACE_UPPER)
    } else {
        (FACE_UPPER, FACE_BLOCK.3 - FACE_UPPER)
    };
    let px = block.px[from * block.w * 4..(from + h) * block.w * 4].to_vec();
    Ok(encode(&Image { w: block.w, h, px }))
}

/// Where a texture coordinate of the layout lands in 1.12.1's atlas.
pub fn remap_uv([u, v]: [f32; 2]) -> [f32; 2] {
    if u < 0.5 {
        [u * 2.0, v]
    } else {
        let fx = (u - 0.5) * 2.0;
        let (bx, by, bw, bh) = FACE_BLOCK;
        let a = ATLAS as f32;
        [
            (bx as f32 + fx * bw as f32) / a,
            (by as f32 + v * bh as f32) / a,
        ]
    }
}

/// Move the texture coordinates of `md20`'s vertices drawn with its body skin (texture type 1)
/// to 1.12.1's atlas ([`remap_uv`]); the others (hair, eyes, glows) keep theirs.
pub fn remap_body_uvs(md20: &mut [u8], skin: &Skin) -> Result<()> {
    let model = crate::model::parse(md20)?;
    let mut body = vec![false; model.vertices.len()];
    for b in &skin.batches {
        let Some(t) = model.texture_of(b.texture_combo) else {
            continue;
        };
        if model.textures[t].kind != 1 {
            continue;
        }
        let Some(s) = skin.sections.get(usize::from(b.section)) else {
            continue;
        };
        let start = s.vertex_start as usize;
        for &v in skin
            .vertices
            .get(start..start + usize::from(s.vertex_count))
            .unwrap_or(&[])
        {
            if let Some(slot) = body.get_mut(usize::from(v)) {
                *slot = true;
            }
        }
    }
    let count = u32::from_le_bytes(md20[0x3c..0x40].try_into().unwrap()) as usize;
    let offset = u32::from_le_bytes(md20[0x40..0x44].try_into().unwrap()) as usize;
    for i in (0..count.min(body.len())).filter(|&i| body[i]) {
        let at = offset + i * 48 + 32;
        let f = |o: usize| f32::from_le_bytes(md20[o..o + 4].try_into().unwrap());
        let [u, v] = remap_uv([f(at), f(at + 4)]);
        md20[at..at + 4].copy_from_slice(&u.to_le_bytes());
        md20[at + 4..at + 8].copy_from_slice(&v.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2:1 image: the body half red, the face square blue.
    fn layout(w: usize) -> Vec<u8> {
        let h = w / 2;
        let mut px = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let c = if x < w / 2 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                };
                px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&c);
            }
        }
        encode(&Image { w, h, px })
    }

    fn at(blp: &[u8], x: u32, y: u32) -> [u8; 4] {
        let d = benilla_blp::decode(blp).unwrap();
        let m = &d.mips[0];
        let i = ((y * m.width + x) * 4) as usize;
        m.rgba[i..i + 4].try_into().unwrap()
    }

    #[test]
    fn the_skin_lands_on_the_atlas_and_its_face_in_the_face_block() {
        let atlas = skin_atlas(&layout(1024)).unwrap();
        let d = benilla_blp::decode(&atlas).unwrap();
        // The chain 1.12.1 reads: log2 of the larger side, past the base.
        assert_eq!((d.width, d.height, d.mips.len()), (256, 256, 8));
        assert_eq!(at(&atlas, 200, 40), [255, 0, 0, 255]);
        assert_eq!(at(&atlas, 64, 200), [0, 0, 255, 255]);
        // A half-size file is the same layout.
        assert_eq!(
            at(&skin_atlas(&layout(512)).unwrap(), 64, 200),
            [0, 0, 255, 255]
        );
    }

    #[test]
    fn a_face_square_splits_into_the_two_strips() {
        let mut px = vec![0u8; 64 * 64 * 4];
        for y in 0..64 {
            for x in 0..64 {
                let c = if y < 21 {
                    [0, 255, 0, 255]
                } else {
                    [0, 0, 0, 255]
                };
                px[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].copy_from_slice(&c);
            }
        }
        let face = encode(&Image { w: 64, h: 64, px });
        let upper = face_tile(&face, true).unwrap();
        let lower = face_tile(&face, false).unwrap();
        let size = |b: &[u8]| {
            let d = benilla_blp::decode(b).unwrap();
            (d.width, d.height)
        };
        assert_eq!((size(&upper), size(&lower)), ((128, 32), (128, 64)));
        assert_eq!(at(&upper, 10, 10), [0, 255, 0, 255]);
        assert_eq!(at(&lower, 10, 40), [0, 0, 0, 255]);
    }

    #[test]
    fn a_coordinate_follows_its_texel() {
        assert_eq!(remap_uv([0.25, 0.5]), [0.5, 0.5]);
        // The face square's corners: the face block's.
        assert_eq!(remap_uv([0.5, 0.0]), [0.0, 160.0 / 256.0]);
        assert_eq!(remap_uv([1.0, 1.0]), [0.5, 1.0]);
    }
}
