//! A Legion (7.3.5) install beside the 1.12.1 data, native only. `WOW_LEGION=<folder>` opens it
//! (`benilla-legion`'s `Library`), which then answers the `mpq://legion/…` paths: a model
//! transcoded to 1.12.1, a texture as it is (`benilla_assets::set_extra_source`). With
//! `WOW_LEGION_DBD=<folder>` (WoWDBDefs' `.dbd` definitions, never copied here), every Legion
//! creature display joins the creature catalog as `DISPLAY_BASE` plus its own id, so a unit of
//! that display draws the Legion model: `WOW_CAPTURE=fxview WOW_FX_DISPLAY=1061830` stands the
//! Fel Lord in the effect viewer. The survivors mode then dresses the creatures it has a Legion
//! look for in it (`survivors::data::legion_looks`).
//!
//! `WOW_LEGION` may instead be a remote copy's URL (`https://…`), read by byte ranges into
//! `WOW_LEGION_CACHE` (`benilla-config/legion-cache` by default), which must hold
//! `Data/data.list`, the names in the copy's `Data/data/`: for a test without the 74 GB install.

use std::path::PathBuf;
use std::sync::Arc;

use bevy::prelude::*;

use benilla_casc::remote::Remote;
use benilla_casc::Dir;
use benilla_legion::library::{display_model_path, AnySource, Library, DISPLAY_BASE};

use super::Creatures;

// The survivors mode's Legion looks are this catalog's ids.
const _: () = assert!(DISPLAY_BASE == crate::survivors::data::LEGION_DISPLAY_BASE);

/// The open install, when `WOW_LEGION` names one.
#[derive(Resource, Clone)]
pub(crate) struct Legion(pub(crate) Arc<Library>);

pub(super) struct LegionPlugin;

impl Plugin for LegionPlugin {
    fn build(&self, app: &mut App) {
        let Some(library) = open() else {
            return;
        };
        let library = Arc::new(library);
        let reader = library.clone();
        benilla_assets::set_extra_source(Box::new(move |path| {
            reader
                .read(path)
                .map(|r| r.map_err(|e| std::io::Error::other(format!("{path}: {e}"))))
        }));
        app.insert_resource(Legion(library))
            .add_systems(Startup, add_displays.after(super::setup_entities));
    }
}

fn open() -> Option<Library> {
    let at = std::env::var("WOW_LEGION").ok().filter(|v| !v.is_empty())?;
    let source: AnySource = if at.starts_with("http://") || at.starts_with("https://") {
        let cache = std::env::var("WOW_LEGION_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("benilla-config/legion-cache"));
        Box::new(Remote {
            url: at.trim_end_matches('/').to_string(),
            cache,
        })
    } else {
        Box::new(Dir(PathBuf::from(&at)))
    };
    let dbd = std::env::var("WOW_LEGION_DBD").ok().map(PathBuf::from);
    match Library::open(source, dbd.as_deref()) {
        Ok(library) => {
            info!("legion: opened {at}");
            Some(library)
        }
        Err(e) => {
            warn!("legion: {at} did not open, no Legion files: {e}");
            None
        }
    }
}

/// Every Legion creature display into the catalog, once it is loaded.
fn add_displays(legion: Res<Legion>, creatures: Option<ResMut<Creatures>>) {
    let Some(mut creatures) = creatures else {
        return;
    };
    let mut added = 0;
    for d in legion.0.displays() {
        if creatures.catalog.add_display(
            DISPLAY_BASE + d.id,
            &display_model_path(d.id),
            d.scale * d.model_scale,
            d.collision_height,
        ) {
            added += 1;
        }
    }
    info!("legion: {added} creature displays, from id {DISPLAY_BASE}");
    // The survivors mode's creatures may wear them now.
    crate::survivors::data::set_legion_looks(added > 0);
}
