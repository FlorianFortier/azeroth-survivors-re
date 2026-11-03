//! Legion's ground on a tile `benilla-legion` wrote (native only): what its terrain adds to
//! 1.12.1's, carried in chunks 1.12.1's reader looks past, and the arrays and table the terrain
//! shader's Legion lane (`terrain.wgsl`, `LEGION_TERRAIN`) reads.
//!
//! - Each ground texture's scale (`MTXP` flags `>> 4 & 0xF`: it spans `2^n` times 1.12.1's
//!   repeat) and its height blend (`MTXP`'s height scale and offset, its `_h` texture's alpha,
//!   named in `MHTX`): a layer shows where its weight times its height stands highest.
//! - Each chunk's vertex colours (`MCCV` at the header's `+0x74`, 145 BGRA, 127 for one), baked
//!   to a 17×17 grid ([`COLOUR_SIZE`]) the shader samples at the chunk's coordinates and doubles.
//!
//! A 1.12.1 tile reads no extras: its tile takes one-texel arrays and the lane off.

use std::collections::HashMap;

use benilla_formats::{blp_bytes_to_native_chain, BlpMipChain, BlpTexels};
use bevy::asset::LoadContext;
use bevy::prelude::*;
use bevy::render::render_resource::ShaderType;

use crate::terrain::{alpha_array_image, layer_array_image, pack_layers, RawLayer, LAYER_TEX_SIZE};

/// The layer-array entries the parameters table holds; a tile's ground textures stay well under.
pub const MAX_LAYERS: usize = 64;

/// A chunk's vertex-colour grid edge: its 9 outer rows and columns and the 8 inner between.
pub const COLOUR_SIZE: u32 = 17;

/// An empty alpha map, for a Legion chunk of one layer: every chunk takes its own alpha slot, so
/// its vertex colours can share the index.
pub(crate) static NO_ALPHA: [u8; 64 * 64 * 4] = [0; 64 * 64 * 4];

/// The shader's table (binding 113): `flags.x` 1 on a Legion tile; per layer-array entry, its
/// scale (`x`, the texture's span in 1.12.1's repeats), height scale (`y`) and offset (`z`).
#[derive(ShaderType, Clone, Debug)]
pub struct LegionTerrainParams {
    pub flags: Vec4,
    pub layers: [Vec4; MAX_LAYERS],
}

impl Default for LegionTerrainParams {
    fn default() -> Self {
        Self {
            flags: Vec4::ZERO,
            layers: [Vec4::new(1.0, 0.0, 1.0, 0.0); MAX_LAYERS],
        }
    }
}

/// A tile's Legion arrays and table; one-texel arrays and the lane off on a 1.12.1 tile.
#[derive(Clone)]
pub struct LegionTile {
    /// The ground textures' height textures, in the layer array's order.
    pub heights: Handle<Image>,
    /// Each chunk's vertex colours, at its alpha index.
    pub colours: Handle<Image>,
    pub params: LegionTerrainParams,
}

/// What a Legion tile carries beyond 1.12.1's.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Extras {
    /// `MTEX`'s names, which the rest are aligned with.
    pub(crate) names: Vec<String>,
    /// Per name: its scale (the texture's span in 1.12.1's repeats), height scale and offset.
    pub(crate) layers: Vec<[f32; 3]>,
    /// Per name: its height texture, empty for none.
    pub(crate) heights: Vec<String>,
    /// Each chunk's 145 BGRA vertex colours, by its `(index x, index y)`.
    pub(crate) colours: HashMap<(u32, u32), Vec<u8>>,
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// An ADT's chunks: magic as written (reversed), payload.
fn chunks(b: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(size) = u32_at(b, at + 4) {
        let start = at + 8;
        let end = start.saturating_add(size as usize).min(b.len());
        out.push((b[at..at + 4].try_into().unwrap(), &b[start..end]));
        at = end;
    }
    out
}

fn names(blob: &[u8]) -> Vec<String> {
    let mut out: Vec<String> = blob
        .split(|&c| c == 0)
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    // The blob ends on a NUL: its last split is no name.
    out.pop();
    out
}

/// A tile's Legion extras, `None` on a tile without them (1.12.1's).
pub(crate) fn read(adt: &[u8]) -> Option<Extras> {
    let all = chunks(adt);
    let find = |m: &[u8; 4]| all.iter().find(|(c, _)| c == m).map(|c| c.1);
    let heights = names(find(b"XTHM")?);
    let names = find(b"XETM").map(names).unwrap_or_default();
    let mtxp = find(b"PXTM").unwrap_or(&[]);
    let layers = (0..names.len())
        .map(|i| {
            let rec = mtxp.get(i * 16..i * 16 + 16);
            let flags = rec.and_then(|r| u32_at(r, 0)).unwrap_or(0);
            let f = |at| {
                rec.and_then(|r| u32_at(r, at))
                    .map(f32::from_bits)
                    .unwrap_or(0.0)
            };
            [(1u32 << ((flags >> 4) & 0xf)) as f32, f(4), f(8)]
        })
        .collect();
    let mut colours = HashMap::new();
    for (_, mcnk) in all.iter().filter(|(c, _)| c == b"KNCM") {
        let (Some(x), Some(y), Some(ofs)) = (u32_at(mcnk, 4), u32_at(mcnk, 8), u32_at(mcnk, 0x74))
        else {
            continue;
        };
        // Counted from the MCNK's magic, as its other sub-chunks are.
        let at = (ofs as usize).saturating_sub(8);
        if ofs != 0 && mcnk.get(at..at + 4) == Some(b"VCCM") {
            if let Some(c) = mcnk.get(at + 8..at + 8 + 145 * 4) {
                colours.insert((x, y), c.to_vec());
            }
        }
    }
    Some(Extras {
        names,
        layers,
        heights,
        colours,
    })
}

/// A chunk's 145 BGRA vertex colours (rows of 9 outer, then 8 inner) as a 17×17 RGBA grid: the
/// outer at even places, the inner at odd ones, the rest each the mean of the two outer beside it.
pub(crate) fn bake_colours(mccv: &[u8]) -> Vec<u8> {
    let n = COLOUR_SIZE as usize;
    let mut grid = vec![[127u8, 127, 127, 255]; n * n];
    let rgb = |i: usize| {
        mccv.get(i * 4..i * 4 + 3)
            .map_or([127, 127, 127, 255], |c| [c[2], c[1], c[0], 255])
    };
    for row in 0..9 {
        for col in 0..9 {
            grid[2 * row * n + 2 * col] = rgb(row * 17 + col);
        }
        if row < 8 {
            for col in 0..8 {
                grid[(2 * row + 1) * n + 2 * col + 1] = rgb(row * 17 + 9 + col);
            }
        }
    }
    let mean = |a: [u8; 4], b: [u8; 4]| {
        [0, 1, 2, 3].map(|k| ((u16::from(a[k]) + u16::from(b[k])) / 2) as u8)
    };
    for r in 0..n {
        for c in 0..n {
            match (r % 2, c % 2) {
                (0, 1) => grid[r * n + c] = mean(grid[r * n + c - 1], grid[r * n + c + 1]),
                (1, 0) => grid[r * n + c] = mean(grid[(r - 1) * n + c], grid[(r + 1) * n + c]),
                _ => {}
            }
        }
    }
    grid.concat()
}

/// The table for a tile whose layer-array entries are `keys` (entry `i + 1` is `keys[i]`, entry
/// 0 the fallback), each found among `extras`' names by `normalize`.
pub(crate) fn params(
    extras: &Extras,
    keys: &[String],
    normalize: impl Fn(&str) -> String,
) -> LegionTerrainParams {
    let mut p = LegionTerrainParams {
        flags: Vec4::new(1.0, 0.0, 0.0, 0.0),
        ..default()
    };
    for (i, key) in keys.iter().enumerate().take(MAX_LAYERS - 1) {
        if let Some(j) = extras.names.iter().position(|n| normalize(n) == *key) {
            let [scale, h, o] = extras.layers[j];
            p.layers[i + 1] = Vec4::new(scale.max(1.0), h, o, 0.0);
        }
    }
    p
}

/// One texel, for a layer with no height texture and an empty array.
fn texel(rgba: [u8; 4]) -> BlpMipChain {
    BlpMipChain {
        width: 1,
        height: 1,
        texels: BlpTexels::Rgba8Unorm,
        mips: vec![rgba.to_vec()],
    }
}

/// A tile's Legion arrays and table: the height textures of its layer-array entries `keys` (by
/// `normalize`d name), and its chunks' vertex colours at their alpha indices (`slots`: alpha
/// index, chunk index x and y) among `alphas`. One-texel arrays and the lane off without extras.
pub(crate) async fn build(
    ctx: &mut LoadContext<'_>,
    extras: Option<&Extras>,
    keys: &[String],
    slots: &[(u32, u32, u32)],
    alphas: u32,
    normalize: impl Fn(&str) -> String,
) -> LegionTile {
    let Some(extras) = extras else {
        return LegionTile {
            heights: ctx.add_labeled_asset(
                "legion_heights".to_string(),
                alpha_array_image(1, 1, vec![0; 4]),
            ),
            colours: ctx.add_labeled_asset(
                "legion_colours".to_string(),
                alpha_array_image(1, 1, vec![127, 127, 127, 255]),
            ),
            params: LegionTerrainParams::default(),
        };
    };
    let mut layers = Vec::with_capacity(keys.len());
    for key in keys {
        let height = extras
            .names
            .iter()
            .position(|n| normalize(n) == *key)
            .and_then(|j| extras.heights.get(j))
            .filter(|h| !h.is_empty());
        let chain = match height {
            Some(h) => ctx
                .read_asset_bytes(format!("mpq://{}", h.replace('\\', "/")))
                .await
                .ok()
                .and_then(|b| blp_bytes_to_native_chain(&b).ok()),
            None => None,
        };
        layers.push(RawLayer {
            chain: chain.unwrap_or_else(|| texel([0, 0, 0, 0])),
            matte: false,
        });
    }
    let count = keys.len() as u32 + 1;
    let heights = layer_array_image(LAYER_TEX_SIZE, count, pack_layers([0; 4], layers));
    let mut colours = vec![127u8; (COLOUR_SIZE * COLOUR_SIZE * 4 * alphas) as usize];
    let per = (COLOUR_SIZE * COLOUR_SIZE * 4) as usize;
    for &(ai, x, y) in slots {
        if let Some(c) = extras.colours.get(&(x, y)) {
            let at = ai as usize * per;
            if let Some(dst) = colours.get_mut(at..at + per) {
                dst.copy_from_slice(&bake_colours(c));
            }
        }
    }
    LegionTile {
        heights: ctx.add_labeled_asset("legion_heights".to_string(), heights),
        colours: ctx.add_labeled_asset(
            "legion_colours".to_string(),
            alpha_array_image(COLOUR_SIZE, alphas, colours),
        ),
        params: params(extras, keys, normalize),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(out: &mut Vec<u8>, magic: &[u8; 4], data: &[u8]) {
        let mut m = *magic;
        m.reverse();
        out.extend_from_slice(&m);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
    }

    #[test]
    fn a_legion_tile_reads_its_scales_heights_and_colours() {
        let mut adt = Vec::new();
        put(&mut adt, b"MTEX", b"Legion\\spec\\1.blp\0Legion\\2.blp\0");
        let mut mtxp = vec![0u8; 32];
        mtxp[0..4].copy_from_slice(&0x20u32.to_le_bytes());
        mtxp[4..8].copy_from_slice(&8.0f32.to_bits().to_le_bytes());
        mtxp[8..12].copy_from_slice(&1.0f32.to_bits().to_le_bytes());
        mtxp[16..20].copy_from_slice(&0x10u32.to_le_bytes());
        put(&mut adt, b"MTXP", &mtxp);
        put(&mut adt, b"MHTX", b"Legion\\3.blp\0\0");
        // One chunk, (2, 5), its colours after the header at 0x74's offset.
        let mut mcnk = vec![0u8; 128];
        mcnk[4..8].copy_from_slice(&2u32.to_le_bytes());
        mcnk[8..12].copy_from_slice(&5u32.to_le_bytes());
        mcnk[0x74..0x78].copy_from_slice(&(8u32 + 128).to_le_bytes());
        let mut mccv = vec![127u8; 145 * 4];
        mccv[0..4].copy_from_slice(&[10, 20, 30, 255]);
        put(&mut mcnk, b"MCCV", &mccv);
        put(&mut adt, b"MCNK", &mcnk);

        let e = read(&adt).unwrap();
        assert_eq!(e.names, ["Legion\\spec\\1.blp", "Legion\\2.blp"]);
        assert_eq!(e.layers, [[4.0, 8.0, 1.0], [2.0, 0.0, 0.0]]);
        assert_eq!(e.heights, ["Legion\\3.blp", ""]);
        assert_eq!(e.colours[&(2, 5)][0..4], [10, 20, 30, 255]);
        // A 1.12.1 tile: no extras.
        let mut plain = Vec::new();
        put(&mut plain, b"MTEX", b"a.blp\0");
        assert_eq!(read(&plain), None);

        // The table, by the layer array's entries (entry 0 the fallback).
        let lower = |s: &str| s.to_ascii_lowercase();
        let p = params(
            &e,
            &["legion\\2.blp".into(), "legion\\spec\\1.blp".into()],
            lower,
        );
        assert_eq!(p.flags.x, 1.0);
        assert_eq!(p.layers[0], Vec4::new(1.0, 0.0, 1.0, 0.0));
        assert_eq!(p.layers[1], Vec4::new(2.0, 0.0, 0.0, 0.0));
        assert_eq!(p.layers[2], Vec4::new(4.0, 8.0, 1.0, 0.0));
    }

    #[test]
    fn vertex_colours_bake_to_their_grid() {
        let mut mccv = vec![0u8; 145 * 4];
        let mut set = |i: usize, v: u8| mccv[i * 4..i * 4 + 4].copy_from_slice(&[v, v, v, 255]);
        // The first row's outer two, and the first inner.
        set(0, 100);
        set(1, 200);
        set(9, 50);
        // The second row's first outer, below the first.
        set(17, 60);
        let g = bake_colours(&mccv);
        let at = |r: usize, c: usize| g[(r * 17 + c) * 4];
        assert_eq!(g.len(), 17 * 17 * 4);
        assert_eq!((at(0, 0), at(0, 2), at(1, 1), at(2, 0)), (100, 200, 50, 60));
        // Between: the two outer beside.
        assert_eq!((at(0, 1), at(1, 0)), (150, 80));
        // BGRA to RGBA.
        let mut one = vec![127u8; 145 * 4];
        one[0..4].copy_from_slice(&[1, 2, 3, 4]);
        assert_eq!(bake_colours(&one)[0..4], [3, 2, 1, 255]);
    }
}
