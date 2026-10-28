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
use benilla_formats::ItemDisplay;
use benilla_legion::library::{
    display_model_path, item_model_name, AnySource, ItemLook, Library, DISPLAY_BASE,
};

use super::{Creatures, ItemDisplays};
use crate::survivors::{data, outfit};

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
        app.insert_resource(Legion(library)).add_systems(
            Startup,
            (add_displays, add_item_displays).after(super::setup_entities),
        );
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
    if added > 0 {
        data::set_legion_looks(true);
    }
}

/// Legion's weapon looks into the item catalog, from `data::LEGION_ITEM_FIRST`: Illidan's
/// glaive first, then each family's (`data::LEGION_WEAPON_FAMILIES`), each with a 1.12.1 icon of
/// its family so the survivors mode's gear draws it among that family's looks.
fn add_item_displays(legion: Res<Legion>, items: Option<ResMut<ItemDisplays>>) {
    let Some(mut items) = items else {
        return;
    };
    let row = |look: &ItemLook, icon: Option<String>| ItemDisplay {
        model: [0, 1].map(|s| (look.models[s] != 0).then(|| item_model_name(look.id, s))),
        icon,
        ..ItemDisplay::default()
    };
    let mut added = 0;
    if let Some(glaive) = legion.0.item_look(data::WARGLAIVE_DISPLAY) {
        // A thrown axe's icon, which no family's pool takes.
        let icon = Some("Interface\\Icons\\INV_ThrowingAxe_01".to_string());
        if items
            .catalog
            .add_display(data::LEGION_ITEM_FIRST, row(&glaive, icon))
        {
            added += 1;
        }
    }
    let looks = legion
        .0
        .weapon_looks(data::LEGION_WEAPON_FAMILIES, data::LEGION_LOOKS_PER_FAMILY);
    for (k, look) in looks.iter().enumerate() {
        let icon = outfit::family_icon(&items.catalog, look.class, look.subclass);
        let id = data::LEGION_ITEM_FIRST + 1 + k as u32;
        if items.catalog.add_display(id, row(look, icon)) {
            added += 1;
        }
    }
    info!(
        "legion: {added} item looks, from id {}",
        data::LEGION_ITEM_FIRST
    );
    if added > 0 {
        data::set_legion_looks(true);
    }
}
