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
        out.push(LegionLight {
            id: r.id,
            pos,
            falloff_start: r.float("GameFalloffStart").unwrap_or(0.0),
            falloff_end: r.float("GameFalloffEnd").unwrap_or(0.0),
            global: pos == [0.0; 3],
            params,
        });
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
