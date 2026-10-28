//! Lights added beside the DBCs' rows: a later client's maps (Legion's, through
//! `benilla-legion`), each light a sphere or a map's global, each set of params its 18 colour
//! bands and 6 float bands in vanilla's layout ([`super::IB_DIFFUSE`] on). Their ids are the
//! caller's, past the DBCs': an id already taken is refused, so the DBCs' rows are never replaced.

use super::tables::Band;
use super::{Light, LightCatalog};

/// A light: a sphere in world yards, or its map's global.
#[derive(Clone, Debug, PartialEq)]
pub struct AddedLight {
    pub id: u32,
    pub map: u32,
    pub pos: [f32; 3],
    /// Yards.
    pub falloff_start: f32,
    pub falloff_end: f32,
    pub global: bool,
    /// Params per weather slot (clear, clear under water, storm, storm under water, death); 0
    /// unset.
    pub params: [u32; 5],
}

/// One set of params: its bands as `(time in half-minutes, value)` keys, a colour `0x00RRGGBB`,
/// and its `LightParams` fields.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AddedParams {
    pub int_bands: [Vec<(u32, u32)>; 18],
    /// The fog end in inches (× 36, as `LightFloatBand` stores it), then the fog start's share of
    /// it, then the cloud density at 3.
    pub float_bands: [Vec<(u32, f32)>; 6],
    pub glow: f32,
    pub highlight_sky: bool,
    /// Water shallow and deep, ocean shallow and deep.
    pub water_alpha: [f32; 4],
}

impl LightCatalog {
    /// Add params `id` (1-based, past the DBCs').
    pub fn add_params(&mut self, id: u32, p: AddedParams) -> bool {
        if id == 0 || self.has_param(id) {
            return false;
        }
        for (b, keys) in p.int_bands.into_iter().enumerate() {
            self.int_bands
                .insert((id - 1) * 18 + b as u32 + 1, Band::new(keys));
        }
        for (b, keys) in p.float_bands.into_iter().enumerate() {
            self.float_bands
                .insert((id - 1) * 6 + b as u32 + 1, Band::new(keys));
        }
        self.light_params_glow.insert(id, p.glow);
        self.light_params_highlight
            .insert(id, if p.highlight_sky { 1.0 } else { 0.0 });
        self.light_params_water_alpha.insert(id, p.water_alpha);
        true
    }

    /// Add a light.
    pub fn add_light(&mut self, l: AddedLight) -> bool {
        if self.lights.iter().any(|o| o.id == l.id) {
            return false;
        }
        self.lights.push(Light {
            id: l.id,
            map: l.map,
            pos: if l.global { [0.0; 3] } else { l.pos },
            falloff_start: l.falloff_start,
            falloff_end: l.falloff_end,
            global: l.global,
            params: l.params,
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty() -> LightCatalog {
        LightCatalog {
            lights: Vec::new(),
            int_bands: HashMap::new(),
            float_bands: HashMap::new(),
            light_params_glow: HashMap::new(),
            light_params_highlight: HashMap::new(),
            light_params_water_alpha: HashMap::new(),
            light_params_skybox: HashMap::new(),
            skyboxes: HashMap::new(),
        }
    }

    #[test]
    fn an_added_maps_global_lights_it_with_its_own_bands() {
        let mut c = empty();
        let mut p = AddedParams::default();
        // A fog colour of pure red all day, the fog ending at 500 yd.
        p.int_bands[super::super::IB_FOG_COLOR as usize] = vec![(0, 0xff0000)];
        p.float_bands[super::super::FB_FOG_END as usize] = vec![(0, 18000.0)];
        assert!(c.add_params(100_001, p.clone()));
        assert!(!c.add_params(100_001, p));
        assert!(c.add_light(AddedLight {
            id: 100_002,
            map: 1220,
            pos: [0.0; 3],
            falloff_start: 0.0,
            falloff_end: 0.0,
            global: true,
            params: [100_001, 0, 0, 0, 0],
        }));
        let a = c.sample(1220, [100.0, 200.0, 0.0], 1440, false);
        assert_eq!(a.fog_color, [1.0, 0.0, 0.0]);
        assert!((a.fog_end - 500.0).abs() < 0.01);
    }
}
