//! The light catalog opened to whoever adds lights to it beside the DBCs' (a later client's maps,
//! Legion's through `benilla-legion`), once [`super::setup_lighting`] has loaded it.

use bevy::prelude::*;

use benilla_formats::LightCatalog;

use super::LightSampler;

/// Run `f` on the loaded light catalog; `false` when there is none (no client data, or before
/// `Startup` has loaded it).
pub fn with_light_catalog(world: &mut World, f: impl FnOnce(&mut LightCatalog)) -> bool {
    match world.get_resource_mut::<LightSampler>() {
        Some(mut sampler) => {
            f(&mut sampler.0);
            true
        }
        None => false,
    }
}
