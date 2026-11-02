//! A Legion map's lights in 1.12.1's layout: its `Light` spheres and global (game coordinates in
//! yards already), and each `LightParams` they name with its `LightData` keys as 1.12.1's 18 colour
//! bands and 6 float bands.
//!
//! The colours carry over as they are: `LightData`'s first eighteen colours are 1.12.1's
//! `LightIntBand` rows in their order (direct, ambient, the five sky stops, the fog, the shadow,
//! the sun, the clouds' four, the ocean's two, the river's two), packed `0x00RRGGBB` both. The fog's
//! distance does not: Legion's fog is a density model whose `FogEnd` is often 0 or a few yards, so
//! an end under [`MIN_FOG_END`] takes [`FOG_END`], and the start is [`FOG_START`] of it, as
//! 1.12.1's outdoor rows have them. The cloud density is Legion's.
//!
//! From Cataclysm a zone's own light is a polygon (`ZoneLight`, its points `ZoneLightPoint`)
//! naming a `Light` row, the map's spheres only its spots. 1.12.1 has spheres alone, and lays the
//! spheres covering a point farthest first, each over the last by its alpha. So each polygon is
//! tiled with a grid of spheres ([`zone_cells`]), every [`CELL`] yards, each full out to
//! [`CELL_FULL`] (past the farthest a point can be from its nearest cell, with room for the
//! ground's rise and fall) and gone at [`CELL_FADE`]: the nearest cell is laid last and wholly, so
//! a point takes its zone's light, a border its nearer side's, and the map's global stays only
//! where no zone is. A cell inside two polygons is the smaller's (a grove within its forest).
//! 1.12.1 measures in three dimensions, so a map's cells all stand at one height, the median of
//! its spots' (the ground they light): the nearest on the ground is then the nearest, and ground
//! within [`CELL_FULL`] of that height, less a cell's reach, is fully its zone's. No cell stands
//! within a spot's reach, so a spot stays the nearest over most of it and lights over its zone.

use std::collections::BTreeMap;
use std::path::Path;

use benilla_casc::{Casc, Source};

use crate::db2::Value;
use crate::library::table;
use crate::Result;

/// A light: a sphere in yards, or the map's global.
#[derive(Clone, Debug, PartialEq)]
pub struct LegionLight {
    pub id: u32,
    pub pos: [f32; 3],
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub global: bool,
    /// `LightParams` ids for 1.12.1's five slots (clear, clear under water, storm, storm under
    /// water, death); 0 unset.
    pub params: [u32; 5],
}

/// One `LightParams` and its keys.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LegionLightParams {
    pub id: u32,
    /// `(time in half-minutes, 0x00RRGGBB)` per colour band.
    pub int_bands: [Vec<(u32, u32)>; 18],
    /// `(time, value)` per float band: the fog end in inches, the fog start's share, 0, the cloud
    /// density, 0, 0.
    pub float_bands: [Vec<(u32, f32)>; 6],
    pub glow: f32,
    pub highlight_sky: bool,
    pub water_alpha: [f32; 4],
}

/// A fog end under this many inches (about 28 yards) is not one 1.12.1 can draw.
pub const MIN_FOG_END: f32 = 1000.0;
/// The fog end then taken, in inches: 600 yards.
pub const FOG_END: f32 = 600.0 * 36.0;
/// The fog start's share of its end.
pub const FOG_START: f32 = 0.25;

/// `LightData`'s colours in 1.12.1's band order.
const COLOURS: [&str; 18] = [
    "DirectColor",
    "AmbientColor",
    "SkyTopColor",
    "SkyMiddleColor",
    "SkyBand1Color",
    "SkyBand2Color",
    "SkySmogColor",
    "SkyFogColor",
    "ShadowOpacity",
    "SunColor",
    "CloudSunColor",
    "CloudEmissiveColor",
    "CloudLayer1AmbientColor",
    "CloudLayer2AmbientColor",
    "OceanCloseColor",
    "OceanFarColor",
    "RiverCloseColor",
    "RiverFarColor",
];

/// A zone light cell's id: this plus its index, past `Light`'s own.
pub const ZONE_LIGHT_BASE: u32 = 50_000;
/// The zone cells' spacing, in yards.
pub const CELL: f32 = 120.0;
/// A zone cell's full strength out to here, and none from here, in yards.
pub const CELL_FULL: f32 = 300.0;
pub const CELL_FADE: f32 = 400.0;

/// Whether `p` lies inside polygon `poly` (even-odd).
pub fn inside(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut inside = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + poly.len() - 1) % poly.len()]);
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            inside = !inside;
        }
    }
    inside
}

/// A polygon's area.
fn area(poly: &[[f32; 2]]) -> f32 {
    let mut twice = 0.0;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        twice += a[0] * b[1] - b[0] * a[1];
    }
    (twice / 2.0).abs()
}

/// The cells of `polygons` on one grid every [`CELL`] yards: `(the polygon's index, the cell's
/// middle)`, each grid point inside one or more polygons given to the smallest of them. A polygon
/// holding no grid point has one cell, at its points' middle.
pub fn zone_cells(polygons: &[Vec<[f32; 2]>]) -> Vec<(usize, [f32; 2])> {
    let areas: Vec<f32> = polygons.iter().map(|p| area(p)).collect();
    let mut cells: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    for (z, poly) in polygons.iter().enumerate().filter(|(_, p)| p.len() >= 3) {
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for p in poly {
            for i in 0..2 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
        for gx in (lo[0] / CELL).floor() as i32..=(hi[0] / CELL).ceil() as i32 {
            for gy in (lo[1] / CELL).floor() as i32..=(hi[1] / CELL).ceil() as i32 {
                if !inside(poly, [gx as f32 * CELL, gy as f32 * CELL]) {
                    continue;
                }
                let held = cells.entry((gx, gy)).or_insert(z);
                if areas[z] < areas[*held] {
                    *held = z;
                }
            }
        }
    }
    let mut out: Vec<(usize, [f32; 2])> = cells
        .into_iter()
        .map(|((gx, gy), z)| (z, [gx as f32 * CELL, gy as f32 * CELL]))
        .collect();
    for (z, poly) in polygons.iter().enumerate().filter(|(_, p)| p.len() >= 3) {
        if !out.iter().any(|c| c.0 == z) {
            let n = poly.len() as f32;
            let mid = [
                poly.iter().map(|p| p[0]).sum::<f32>() / n,
                poly.iter().map(|p| p[1]).sum::<f32>() / n,
            ];
            out.push((z, mid));
        }
    }
    out
}

/// Map `map`'s lights and the params they name, from the install's tables.
pub fn map_lights<S: Source>(
    casc: &Casc<S>,
    dir: &Path,
    map: u32,
) -> Result<(Vec<LegionLight>, Vec<LegionLightParams>)> {
    let lights = table(casc, dir, "Light")?;
    let mut out = Vec::new();
    for r in lights
        .rows
        .values()
        .filter(|r| r.int("ContinentID") == Some(i64::from(map)))
    {
        let pos = match r.get("GameCoords") {
            Some(Value::Floats(f)) if f.len() == 3 => [f[0], f[1], f[2]],
            _ => continue,
        };
        let ids = r.ints("LightParamsID");
        let mut params = [0u32; 5];
        for (p, id) in params.iter_mut().zip(ids) {
            *p = id.max(0) as u32;
        }
        debug_assert!(r.id < ZONE_LIGHT_BASE);
        out.push(LegionLight {
            id: r.id,
            pos,
            falloff_start: r.float("GameFalloffStart").unwrap_or(0.0),
            falloff_end: r.float("GameFalloffEnd").unwrap_or(0.0),
            global: pos == [0.0; 3],
            params,
        });
    }
    // The zones' lights, where the definitions name their tables.
    if let (Ok(zones), Ok(points)) = (
        table(casc, dir, "ZoneLight"),
        table(casc, dir, "ZoneLightPoint"),
    ) {
        let mut polygons: BTreeMap<u32, Vec<(i64, [f32; 2])>> = BTreeMap::new();
        for p in points.rows.values() {
            if let (Some(zone), Some(Value::Floats(f))) = (p.int("ZoneLightID"), p.get("Pos")) {
                if f.len() == 2 {
                    let order = p.int("PointOrder").unwrap_or(0);
                    polygons
                        .entry(zone as u32)
                        .or_default()
                        .push((order, [f[0], f[1]]));
                }
            }
        }
        // One height for the map's cells: the median of its spots'.
        let mut heights: Vec<f32> = out.iter().filter(|l| !l.global).map(|l| l.pos[2]).collect();
        heights.sort_by(f32::total_cmp);
        let height = heights.get(heights.len() / 2).copied().unwrap_or(0.0);
        // Each zone: its polygon and its light's params.
        let mut shapes = Vec::new();
        let mut kinds = Vec::new();
        for z in zones
            .rows
            .values()
            .filter(|z| z.int("MapID") == Some(i64::from(map)))
        {
            let Some(light) = z.int("LightID").and_then(|l| lights.get(l as u32)) else {
                continue;
            };
            let mut poly = polygons.remove(&z.id).unwrap_or_default();
            poly.sort_by_key(|p| p.0);
            let mut params = [0u32; 5];
            for (p, id) in params.iter_mut().zip(light.ints("LightParamsID")) {
                *p = id.max(0) as u32;
            }
            shapes.push(poly.into_iter().map(|p| p.1).collect::<Vec<_>>());
            kinds.push(params);
        }
        let spots: Vec<([f32; 3], f32)> = out
            .iter()
            .filter(|l| !l.global)
            .map(|l| (l.pos, l.falloff_end))
            .collect();
        let clear = |at: [f32; 2]| {
            spots
                .iter()
                .all(|(p, end)| (p[0] - at[0]).powi(2) + (p[1] - at[1]).powi(2) >= end * end)
        };
        let cells = zone_cells(&shapes).into_iter().filter(|c| clear(c.1));
        for (i, (z, at)) in cells.enumerate() {
            let params = kinds[z];
            out.push(LegionLight {
                id: ZONE_LIGHT_BASE + i as u32,
                pos: [at[0], at[1], height],
                falloff_start: CELL_FULL,
                falloff_end: CELL_FADE,
                global: false,
                params,
            });
        }
    }
    let wanted: std::collections::BTreeSet<u32> = out
        .iter()
        .flat_map(|l| l.params)
        .filter(|&p| p != 0)
        .collect();
    let params_table = table(casc, dir, "LightParams")?;
    let data = table(casc, dir, "LightData")?;
    let mut keys: BTreeMap<u32, Vec<&crate::db2::Row>> = BTreeMap::new();
    for d in data.rows.values() {
        if let Some(p) = d
            .int("LightParamID")
            .filter(|&p| wanted.contains(&(p as u32)))
        {
            keys.entry(p as u32).or_default().push(d);
        }
    }
    let mut params = Vec::new();
    for &id in &wanted {
        let Some(rows) = keys.get_mut(&id) else {
            continue;
        };
        rows.sort_by_key(|d| d.int("Time").unwrap_or(0));
        let mut p = LegionLightParams {
            id,
            ..Default::default()
        };
        for d in rows.iter() {
            let t = d.int("Time").unwrap_or(0).clamp(0, 2879) as u32;
            for (b, name) in COLOURS.iter().enumerate() {
                let c = d.int(name).unwrap_or(0) as u32 & 0x00ff_ffff;
                p.int_bands[b].push((t, c));
            }
            let end = d.float("FogEnd").unwrap_or(0.0);
            let end = if end < MIN_FOG_END { FOG_END } else { end };
            p.float_bands[0].push((t, end));
            p.float_bands[1].push((t, FOG_START));
            p.float_bands[3].push((t, d.float("CloudDensity").unwrap_or(0.0)));
        }
        if let Some(r) = params_table.get(id) {
            p.glow = r.float("Glow").unwrap_or(0.0);
            p.highlight_sky = r.int("HighlightSky").unwrap_or(0) != 0;
            p.water_alpha = [
                r.float("WaterShallowAlpha").unwrap_or(0.0),
                r.float("WaterDeepAlpha").unwrap_or(0.0),
                r.float("OceanShallowAlpha").unwrap_or(0.0),
                r.float("OceanDeepAlpha").unwrap_or(0.0),
            ];
        }
        params.push(p);
    }
    Ok((out, params))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_are_tiled_each_cell_to_its_smallest_polygon() {
        let square = |x0: f32, y0: f32, side: f32| {
            vec![
                [x0, y0],
                [x0 + side, y0],
                [x0 + side, y0 + side],
                [x0, y0 + side],
            ]
        };
        // A forest of 1,000 yards, a grove of 300 inside it, a second forest beside.
        let zones = [
            square(0.0, 0.0, 1000.0),
            square(300.0, 300.0, 300.0),
            square(1000.0, 0.0, 500.0),
        ];
        let cells = zone_cells(&zones);
        let of = |z: usize| cells.iter().filter(|c| c.0 == z).count();
        assert!(of(0) > 50 && of(1) > 3 && of(2) > 10);
        for &(z, p) in &cells {
            assert!(inside(&zones[z], p));
            // The grove holds its own cells.
            if inside(&zones[1], p) {
                assert_eq!(z, 1);
            }
        }
        // Every point of the forest is nearer one of its cells than a cell's full strength.
        let worst = (CELL * CELL / 2.0).sqrt();
        assert!(worst < CELL_FULL && CELL_FULL < CELL_FADE);
        // A polygon too small to hold a grid point: one cell, at its middle.
        let small = [vec![[10.0, 10.0], [40.0, 10.0], [40.0, 40.0]]];
        assert_eq!(zone_cells(&small), [(0, [30.0, 20.0])]);
    }
}
