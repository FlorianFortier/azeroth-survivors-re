//! A Legion (7.3.5) install beside the 1.12.1 data, native only. `WOW_LEGION=<folder>` opens it
//! (`benilla-legion`'s `Library`), which then answers the `legion/…` paths beside the patch
//! chain's archives (`benilla_formats::set_extra_files`): a model transcoded to 1.12.1, a texture
//! as it is. With
//! `WOW_LEGION_DBD=<folder>` (WoWDBDefs' `.dbd` definitions, never copied here), every Legion
//! creature display joins the creature catalog as `DISPLAY_BASE` plus its own id, so a unit of
//! that display draws the Legion model: `WOW_CAPTURE=fxview WOW_FX_DISPLAY=1061830` stands the
//! Fel Lord in the effect viewer. The survivors mode then dresses the creatures it has a Legion
//! look for in it (`survivors::data::legion_looks`).
//!
//! Legion's playable races join the character tables in their pre-HD bodies
//! (`survivors::data::LEGION_RACES`): their body displays (a `Character\…` model path, so the
//! client composites them), their skin, face and hair textures at 1.12.1's size, their geosets
//! and their create-screen entry with its dial ranges.
//!
//! Legion's classes (`survivors::data::LEGION_CLASSES`) bring their spells' names and icons
//! (`survivors::data::legion_spell`), their icon, and their specializations as talent tabs
//! beside 1.12.1's.
//!
//! Legion's maps (`benilla_legion::library::LEGION_MAPS`: the Broken Isles) join the client's
//! map table, their terrain, doodads, buildings and water answered by the library as 1.12.1
//! tiles, their lights added to the client's light catalog.
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
    display_model_path, item_model_name, AnySource, ItemLook, Library, DISPLAY_BASE, LEGION_MAPS,
};

use benilla_formats::DialRanges;
use benilla_legion::races::{LegionRace, SectionKey};

use super::skin_composite::SkinSections;
use super::{CharCreate, Characters, Creatures, ItemDisplays};
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
        benilla_formats::set_extra_files(Box::new(Files(library.clone())));
        app.insert_resource(Legion(library))
            .add_systems(
                Startup,
                (add_displays, add_item_displays, add_races).after(super::setup_entities),
            )
            // After every startup system, the talent catalog's loading among them.
            .add_systems(PostStartup, (add_classes, add_maps, add_lights));
    }
}

/// The install as files beside the archives.
struct Files(Arc<Library>);

impl benilla_formats::ExtraFiles for Files {
    fn claims(&self, path: &str) -> bool {
        self.0.claims(path)
    }

    fn read(&self, path: &str) -> std::io::Result<Vec<u8>> {
        match self.0.read(path) {
            Some(r) => r.map_err(|e| std::io::Error::other(format!("{path}: {e}"))),
            None => Err(std::io::Error::other(format!("{path}: not a Legion file"))),
        }
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

/// Legion's playable races into the character tables, in their pre-HD bodies.
fn add_races(
    legion: Res<Legion>,
    creatures: Option<ResMut<Creatures>>,
    sections: Option<ResMut<SkinSections>>,
    geosets: Option<ResMut<Characters>>,
    create: Option<ResMut<CharCreate>>,
) {
    let (Some(mut creatures), Some(mut sections), Some(mut geosets), Some(mut create)) =
        (creatures, sections, geosets, create)
    else {
        return;
    };
    let mut added = Vec::new();
    for id in data::LEGION_RACES {
        let Some(race) = legion.0.race(id) else {
            continue;
        };
        if race.models.contains(&0) {
            warn!("legion: race {id} has no body model");
            continue;
        }
        let mut bodies = [0u32; 2];
        for (sex, body) in bodies.iter_mut().enumerate() {
            let display = DISPLAY_BASE + race.displays[sex];
            let path = format!(
                "Character\\{}\\{}\\legion_char_{}.mdx",
                race.file,
                if sex == 0 { "Male" } else { "Female" },
                race.models[sex]
            );
            creatures.catalog.add_display(display, &path, 1.0, 0.0);
            *body = display;
        }
        for &((sex, section, variation, color), files) in &race.sections {
            let mut paths = files.map(|f| {
                if f == 0 {
                    String::new()
                } else {
                    format!("Legion\\Half\\{f}.blp")
                }
            });
            // The Pandaren's layout: its skin as 1.12.1's atlas, its one face square cut into
            // the face's two tiles (the lower first, as 1.12.1's face rows hold them).
            if race.layout == 2 {
                match (section, files) {
                    (0, [f, ..]) if f != 0 => paths[0] = format!("Legion\\Layout2\\Skin\\{f}.blp"),
                    (1, [a, b, _]) if a != 0 || b != 0 => {
                        let f = if b != 0 { b } else { a };
                        paths[0] = format!("Legion\\Layout2\\Face\\{f}_lower.blp");
                        paths[1] = format!("Legion\\Layout2\\Face\\{f}_upper.blp");
                    }
                    _ => {}
                }
            }
            sections
                .tables
                .add_section((id, sex, section, variation, color), paths);
        }
        for &((sex, style), geoset) in &race.hair {
            geosets.0.add_hair((id, sex, style), geoset);
        }
        for &((sex, style), g) in &race.facial {
            geosets.0.add_facial((id, sex, style), g);
        }
        create.0.add_race(
            id,
            (bodies[0], bodies[1]),
            &race.file,
            data::race_classes(id),
            [0, 1].map(|sex| ranges(&race, sex)),
        );
        added.push(id);
    }
    info!("legion: races {added:?}");
    // Their icons, Legion's own: `Achievement_Character_<Race>_<Sex>`.
    let icons = added
        .iter()
        .flat_map(|&id| [(id, 0u8), (id, 1u8)])
        .filter_map(|(id, sex)| {
            let file = legion.0.race(id)?.file;
            let path = format!(
                "interface/icons/achievement_character_{}_{}.blp",
                file.to_lowercase(),
                if sex == 0 { "male" } else { "female" }
            );
            let fdid = legion.0.file_id(&path)?;
            Some(((id, sex), format!("Legion\\{fdid}.blp")))
        })
        .collect();
    data::set_legion_race_icons(icons);
    data::set_legion_races(&added);
}

/// A race's five dial counts for `sex`, counted as 1.12.1's create screen offers them: skin
/// colours, faces (on the first skin), hair styles, hair colours (of the first style), facial
/// hair.
fn ranges(race: &LegionRace, sex: u8) -> Option<DialRanges> {
    let count = |f: &dyn Fn(&SectionKey) -> bool| {
        race.sections
            .iter()
            .filter(|(k, t)| k.0 == sex && t[0] != 0 && f(k))
            .count()
    };
    let skin = count(&|k| k.1 == 0 && k.2 == 0);
    let face = count(&|k| k.1 == 1 && k.3 == 0);
    let hair_color = count(&|k| k.1 == 3 && k.2 == 0);
    let hair_style = race.hair.iter().filter(|(k, _)| k.0 == sex).count();
    let facial = race.facial.iter().filter(|(k, _)| k.0 == sex).count();
    let byte = |n: usize| n.clamp(1, 255) as u8;
    (skin > 0).then(|| DialRanges {
        skin: byte(skin),
        face: byte(face),
        hair_style: byte(hair_style),
        hair_color: byte(hair_color),
        facial_hair: byte(facial),
    })
}

/// Legion's maps the library answers ([`LEGION_MAPS`]) join the client's map table, each at its
/// Legion id in a directory of its own, whose tile table and tiles the library writes as 1.12.1's.
fn add_maps(maps: Option<ResMut<benilla_assets::MapCatalogRes>>) {
    let Some(mut maps) = maps else {
        return;
    };
    for m in LEGION_MAPS {
        maps.0.add_map(m.id, m.ours, m.name);
    }
    // Their zones stand on Legion's creatures, so they are offered where those are drawn.
    data::set_legion_zones(data::legion_looks());
    info!(
        "legion: maps {:?}",
        LEGION_MAPS.iter().map(|m| m.id).collect::<Vec<_>>()
    );
}

/// A Legion light's and light params' id in the client's catalog: this plus its own, past
/// 1.12.1's.
pub(crate) const LIGHT_BASE: u32 = 100_000;

/// Legion's maps' lights join the client's catalog: each map's global and spheres, each params'
/// colours and fog in 1.12.1's bands (`benilla_legion::lights`).
fn add_lights(world: &mut World) {
    let Some(legion) = world.get_resource::<Legion>().map(|l| l.0.clone()) else {
        return;
    };
    let mut read = Vec::new();
    for m in LEGION_MAPS {
        match legion.map_lights(m.id) {
            Ok(l) => read.push((m.id, l)),
            Err(e) => warn!("legion: map {} lights: {e}", m.id),
        }
    }
    let added = benilla_world::lighting::with_light_catalog(world, |catalog| {
        for (map, (lights, params)) in read {
            for p in params {
                catalog.add_params(
                    LIGHT_BASE + p.id,
                    benilla_formats::AddedParams {
                        int_bands: p.int_bands,
                        float_bands: p.float_bands,
                        glow: p.glow,
                        highlight_sky: p.highlight_sky,
                        water_alpha: p.water_alpha,
                    },
                );
            }
            let count = lights.len();
            for l in lights {
                catalog.add_light(benilla_formats::AddedLight {
                    id: LIGHT_BASE + l.id,
                    map,
                    pos: l.pos,
                    falloff_start: l.falloff_start,
                    falloff_end: l.falloff_end,
                    global: l.global,
                    params: l.params.map(|p| if p == 0 { 0 } else { LIGHT_BASE + p }),
                });
            }
            info!("legion: map {map}, {count} lights");
        }
    });
    if !added {
        warn!("legion: no light catalog to add the maps' lights to");
    }
}

/// A Legion specialization's talent tab id in the client's catalog: this plus its own, past
/// 1.12.1's tabs; a talent's likewise.
pub(crate) const TALENT_BASE: u32 = 100_000;

/// Points a talent of tiers 1 to 6 takes, so that a tree of seven tiers of three opens its next
/// tier at five points, as vanilla's do; the seventh tier's are one point, exceptional.
const TALENT_RANKS: usize = 3;

/// Legion's classes: their specializations as talent tabs, their spells' names and icons, their
/// own icons. A class whose specializations the install lacks is not offered.
fn add_classes(legion: Res<Legion>, talents: Option<ResMut<crate::ui_talent::Talents>>) {
    let library = &legion.0;
    let mut spells: std::collections::HashMap<u32, data::LegionSpell> = Default::default();
    let mut note = |id: u32| {
        if spells.contains_key(&id) {
            return true;
        }
        let Some(s) = library.spell(id).filter(|s| s.icon != 0) else {
            return false;
        };
        spells.insert(
            id,
            data::LegionSpell {
                name: s.name,
                icon: format!("Legion\\{}", s.icon),
            },
        );
        true
    };
    let own: Vec<u32> = data::legion_spell_ids();
    if !own.iter().all(|&id| note(id)) {
        warn!("legion: its class spells are not all in the install; no Legion classes");
        return;
    }
    let mut catalog = talents;
    let mut added = Vec::new();
    for class in data::LEGION_CLASSES {
        let specs = library.specs(class);
        if specs.is_empty() {
            continue;
        }
        for spec in &specs {
            let tab = benilla_formats::TalentTabInfo {
                id: TALENT_BASE + spec.id,
                name: spec.name.clone(),
                race_mask: u32::MAX,
                class_mask: 1 << (class - 1),
                background: format!("{}{}", class_file(class), spec.name.replace(' ', "")),
            };
            let list = spec
                .talents
                .iter()
                .filter(|t| note(t.spell))
                .map(|t| {
                    let last = t.tier == 6;
                    let n = if last { 1 } else { TALENT_RANKS };
                    let mut ranks = [0; benilla_formats::MAX_TALENT_RANK];
                    ranks[..n].fill(t.spell);
                    benilla_formats::Talent {
                        id: TALENT_BASE + t.id,
                        tab: tab.id,
                        row: u32::from(t.tier),
                        col: u32::from(t.column),
                        ranks,
                        prereq_talent: 0,
                        prereq_rank: 0,
                        required_spell: 0,
                        exceptional: last,
                    }
                })
                .collect();
            if let Some(c) = catalog.as_deref_mut() {
                c.catalog.add_tab(tab, list);
            }
        }
        added.push(class);
    }
    let icons = added
        .iter()
        .filter_map(|&class| {
            let path = format!(
                "interface/icons/classicon_{}.blp",
                class_file(class).to_lowercase()
            );
            Some((class, format!("Legion\\{}.blp", library.file_id(&path)?)))
        })
        .collect();
    info!("legion: classes {added:?}, {} spells", spells.len());
    data::set_legion_spells(spells);
    data::set_legion_class_icons(icons);
    data::set_legion_classes(&added);
}

/// A Legion class's file name, as its icons and talent art name it.
fn class_file(class: u8) -> &'static str {
    match class {
        6 => "DeathKnight",
        10 => "Monk",
        12 => "DemonHunter",
        _ => "",
    }
}
