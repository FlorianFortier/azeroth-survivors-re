//! A Legion install as the client's files: the paths under `legion/` a 1.12.1 asset reader can ask
//! for, each answered from the install, a model transcoded to 1.12.1 ([`crate::transcode`]) and a
//! texture as it is (Legion's BLPs are 1.12.1's BLP2).
//!
//! - `legion/<file data id>.blp`: a texture, the name a transcoded model gives its own;
//! - `legion/<file data id>.m2`: a model, its body and each geoset group's first variant shown,
//!   the textures its display would give left out;
//! - `legion/display/<display id>.m2`: a creature display's model in its own textures and geosets,
//!   from `CreatureDisplayInfo` (WoWDBDefs' definitions needed, [`Library::open`]).
//!
//! - `legion/half/<file data id>.blp`: a texture at half its size (a race's skins, for 1.12.1's
//!   256² body atlas, [`crate::races`]);
//! - `…/legion_char_<file data id>.m2`, in any folder: a character body, every geoset kept for the
//!   client's own selection;
//! - `…/legion_item_<item display id>_<side>.m2`, in any folder: an item display's model (its
//!   left or right one), its skin from the display's material ([`item_model_name`]).
//!
//! A model comes with its keys, its `.anim` files read in, and, with the definitions, the
//! fallbacks `AnimationData` gives the animations it lacks.
//!
//! With the definitions, the item tables give each item display its model and skin files
//! (`ItemDisplayInfo` → `ModelFileData`, `TextureFileData`), and each weapon its family (`Item`'s
//! class and subclass, through `ItemModifiedAppearance` and `ItemAppearance`): [`ItemLook`],
//! [`Library::weapon_looks`]. The class tables give a spell's name and icon and each class's
//! specializations with their talents ([`crate::classes`]).
//!
//! - `world/maps/<map>/<map>.wdt` and `…/<map>_<x>_<y>.adt`, for each of [`LEGION_MAPS`]: its
//!   tile table and its tiles, each written as one 1.12.1 ADT ([`crate::terrain`]).
//!
//! The paths come as the asset reader holds them: lowercase, `/` between folders. A creature
//! display joins the client's catalog as [`DISPLAY_BASE`] plus its Legion id, its model named
//! [`display_model_path`].

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use benilla_casc::{Casc, Source};

use crate::db2::{Raw, Table};
use crate::dbd::Definition;
use crate::display::{self, CreatureDisplay};
use crate::transcode::{self, virtual_path, Animation};
use crate::{md21, model, Error, Result};

/// The build the install is, for the definitions' layouts.
pub const BUILD: &str = "7.3.5.26972";
/// The first path a Legion file has.
pub const PREFIX: &str = "legion/";
/// A Legion creature display's id in the client's catalog: this plus its own. 1.12.1's ids stop
/// under 16,000 and Legion's under 100,000.
pub const DISPLAY_BASE: u32 = 1_000_000;

/// The model path a Legion display has in the client's catalog: `.mdx`, as 1.12.1's rows name
/// theirs, which the client's model loader reads as `legion/display/<id>.m2`.
pub fn display_model_path(id: u32) -> String {
    format!("Legion\\Display\\{id}.mdx")
}

/// The model file name the client is given for item display `id`'s `side` (0 left, 1 right);
/// [`Library::read`] answers it in whatever folder the client puts it.
pub fn item_model_name(id: u32, side: usize) -> String {
    format!("legion_item_{id}_{side}.m2")
}

/// An item display's look: its models and their skins, left and right (0 for none), and the
/// class and subclass of an item that wears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemLook {
    pub id: u32,
    pub class: u8,
    pub subclass: u8,
    pub models: [u32; 2],
    pub textures: [u32; 2],
}

/// The item tables, as far as looks go.
struct Items {
    displays: Table,
    /// `ModelResourcesID` to its first model file.
    models: HashMap<u32, u32>,
    /// `ModelMaterialResourcesID` to its first texture file.
    textures: HashMap<u32, u32>,
    /// Each item display some item wears, with that item's class and subclass and the item's id
    /// (the lowest item wearing it).
    worn: BTreeMap<u32, (u8, u8, u32)>,
}

/// A source any thread may read: an install on disk or a remote copy.
pub type AnySource = Box<dyn Source + Send + Sync>;

/// The creature display tables.
struct Displays {
    info: Table,
    models: Table,
}

/// An open install and, with the definitions, its creature displays and its animations'
/// fallbacks.
pub struct Library {
    casc: Casc<AnySource>,
    displays: Option<Displays>,
    /// `AnimationData`'s `Fallback`, by animation id; empty without the definitions.
    fallbacks: Vec<u16>,
    items: Option<Items>,
    races: Option<crate::races::Races>,
    classes: Option<crate::classes::Classes>,
}

impl Library {
    /// Open the install; `dbd` is a folder of WoWDBDefs' `.dbd` definitions, which the creature
    /// displays need.
    pub fn open(source: AnySource, dbd: Option<&Path>) -> Result<Self> {
        let casc = Casc::open(source)?;
        let (displays, fallbacks) = match dbd {
            Some(dir) => {
                let animations = table(&casc, dir, "AnimationData")?;
                let count = animations.rows.keys().max().map_or(0, |&m| m as usize + 1);
                let mut fallbacks = vec![0u16; count];
                for (&id, row) in &animations.rows {
                    fallbacks[id as usize] = row.int("Fallback").unwrap_or(0) as u16;
                }
                let displays = Displays {
                    info: table(&casc, dir, "CreatureDisplayInfo")?,
                    models: table(&casc, dir, "CreatureModelData")?,
                };
                (Some(displays), fallbacks)
            }
            None => (None, Vec::new()),
        };
        let items = dbd.map(|dir| items(&casc, dir)).transpose()?;
        let races = dbd
            .map(|dir| crate::races::Races::load(&casc, dir))
            .transpose()?;
        let classes = dbd
            .map(|dir| crate::classes::Classes::load(&casc, dir))
            .transpose()?;
        Ok(Self {
            casc,
            displays,
            fallbacks,
            items,
            races,
            classes,
        })
    }

    /// The file at `path`, `None` when it is not a Legion path.
    pub fn read(&self, path: &str) -> Option<Result<Vec<u8>>> {
        // A map's tile table or one of its tiles.
        if let Some(r) = self.map_file(path) {
            return Some(r);
        }
        // An item's model, wherever the client looks for it.
        let base = path.rsplit('/').next().unwrap_or(path);
        // A character body, every geoset kept for the client's own selection.
        if let Some(rest) = base.strip_prefix("legion_char_") {
            return Some(
                match rest.strip_suffix(".m2").and_then(|r| r.parse::<u32>().ok()) {
                    Some(file) => self.transcode(file, |_| None, |_| true),
                    None => Err(Error::Format(format!("no character model at {path}"))),
                },
            );
        }
        if let Some(rest) = base.strip_prefix("legion_item_") {
            let parsed = rest
                .strip_suffix(".m2")
                .and_then(|r| r.rsplit_once('_'))
                .and_then(|(id, side)| {
                    Some((id.parse::<u32>().ok()?, side.parse::<usize>().ok()?))
                });
            return Some(match parsed {
                Some((id, side)) if side < 2 => self.item_model(id, side),
                _ => Err(Error::Format(format!("no item model at {path}"))),
            });
        }
        let rest = path.strip_prefix(PREFIX)?;
        let number = |s: &str, ext: &str| s.strip_suffix(ext)?.parse::<u32>().ok();
        Some(if let Some(id) = rest.strip_prefix("display/") {
            let Some(id) = number(id, ".m2") else {
                return Some(Err(Error::Format(format!("no display at {path}"))));
            };
            self.display(id)
                .ok_or_else(|| Error::Format(format!("no creature display {id}")))
                .and_then(|d| self.model(d.model_file, Some(&d)))
        } else if let Some(file) = number(rest, ".m2") {
            self.model(file, None)
        } else if let Some(file) = rest.strip_prefix("half/").and_then(|r| number(r, ".blp")) {
            self.casc
                .read_id(file)
                .map(|b| crate::races::half_blp(&b))
                .map_err(Error::from)
        } else if let Some(file) = number(rest, ".blp") {
            self.casc.read_id(file).map_err(Error::from)
        } else {
            Err(Error::Format(format!("no Legion file at {path}")))
        })
    }

    /// A file's data id by its path (`/` or `\\`, any case).
    pub fn file_id(&self, path: &str) -> Option<u32> {
        self.casc.id_of(&path.replace('\\', "/"))
    }

    /// Whether `path` (lowercase, `/` between folders) is one [`Self::read`] answers.
    pub fn claims(&self, path: &str) -> bool {
        let base = path.rsplit('/').next().unwrap_or(path);
        map_path(path).is_some()
            || path.starts_with(PREFIX)
            || base.starts_with("legion_item_")
            || base.starts_with("legion_char_")
    }

    /// Playable race `id` in its pre-HD body; none without the definitions.
    pub fn race(&self, id: u8) -> Option<crate::races::LegionRace> {
        let (races, items) = (self.races.as_ref()?, self.items.as_ref()?);
        races.race(
            id,
            |m| items.textures.get(&m).copied().unwrap_or(0),
            |d| self.display(d).map_or(0, |d| d.model_file),
        )
    }

    /// A map's file at `path`: its WDT, or a tile written as one 1.12.1 ADT
    /// ([`crate::terrain`]).
    fn map_file(&self, path: &str) -> Option<Result<Vec<u8>>> {
        let (map, file) = map_path(path)?;
        let at = |suffix: &str| format!("world/maps/{}/{}{suffix}", map.legion, map.legion);
        let wdt = || self.casc.read_path(&at(".wdt")).map_err(Error::from);
        Some(match file {
            MapFile::Wdt => wdt().and_then(|w| crate::terrain::wdt(&w)),
            MapFile::Tile(x, y) => (|| {
                let big = crate::terrain::big_alpha(&wdt()?);
                let tile = |part: &str| self.casc.read_path(&at(&format!("_{x}_{y}{part}.adt")));
                let (root, tex0) = (tile("")?, tile("_tex0")?);
                let obj0 = tile("_obj0").ok();
                // A name the install holds no data for is left out.
                let resolve = |p: &str| self.file_id(p).filter(|&id| self.casc.has_id(id));
                crate::terrain::adt(&root, &tex0, obj0.as_deref(), big, &resolve)
            })(),
        })
    }

    /// Spell `id`'s name and icon; none without the definitions.
    pub fn spell(&self, id: u32) -> Option<crate::classes::LegionSpell> {
        self.classes.as_ref()?.spell(id)
    }

    /// Class `class`'s specializations and their talents; none without the definitions.
    pub fn specs(&self, class: u8) -> Vec<crate::classes::LegionSpec> {
        self.classes
            .as_ref()
            .map_or_else(Vec::new, |c| c.specs(class))
    }

    /// A creature display by its Legion id.
    pub fn display(&self, id: u32) -> Option<CreatureDisplay> {
        let d = self.displays.as_ref()?;
        display::creature_display(&d.info, &d.models, id)
    }

    /// Every creature display, by id; none without the definitions.
    pub fn displays(&self) -> impl Iterator<Item = CreatureDisplay> + '_ {
        self.displays.iter().flat_map(|d| {
            d.info
                .rows
                .keys()
                .filter_map(|&id| display::creature_display(&d.info, &d.models, id))
        })
    }

    /// An item display's look; none without the definitions.
    pub fn item_look(&self, id: u32) -> Option<ItemLook> {
        let items = self.items.as_ref()?;
        let row = items.displays.get(id)?;
        let pick = |ids: Vec<i64>, map: &HashMap<u32, u32>| {
            [0, 1].map(|s| {
                ids.get(s)
                    .and_then(|&r| map.get(&(r as u32)))
                    .copied()
                    .unwrap_or(0)
            })
        };
        let (class, subclass, _) = items.worn.get(&id).copied().unwrap_or((0, 0, 0));
        Some(ItemLook {
            id,
            class,
            subclass,
            models: pick(row.ints("ModelResourcesID"), &items.models),
            textures: pick(row.ints("ModelMaterialResourcesID"), &items.textures),
        })
    }

    /// Up to `per_family` of Legion's own weapon looks for each `(class, subclass)` of
    /// `families`, in that order: looks worn by an item of Legion's (id from 121,000) whose model
    /// was made for it (file data id from 1,100,000) and skinned by the display, one per model,
    /// the lowest display ids first. The same list wherever the same install is read.
    pub fn weapon_looks(&self, families: &[(u8, u8)], per_family: usize) -> Vec<ItemLook> {
        let Some(items) = self.items.as_ref() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &family in families {
            let mut models = HashSet::new();
            let looks = items
                .worn
                .iter()
                .filter(|(_, &(c, s, item))| (c, s) == family && item >= 121_000)
                .filter_map(|(&id, _)| self.item_look(id))
                .filter(|l| l.models[0] >= 1_100_000 && l.textures[0] != 0)
                .filter(|l| models.insert(l.models[0]))
                .take(per_family);
            out.extend(looks);
        }
        out
    }

    /// Item display `id`'s `side` model as a 1.12.1 MD20, its skin (texture type 2) the
    /// display's.
    pub fn item_model(&self, id: u32, side: usize) -> Result<Vec<u8>> {
        let look = self
            .item_look(id)
            .ok_or_else(|| Error::Format(format!("no item display {id}")))?;
        let file = look.models[side];
        if file == 0 {
            return Err(Error::Format(format!(
                "item display {id} has no model {side}"
            )));
        }
        let skin = look.textures[side];
        self.transcode(
            file,
            |kind| (kind == 2 && skin != 0).then_some(skin),
            |g| g == 0 || g % 100 == 1,
        )
    }

    /// The model in file `file` as a 1.12.1 MD20, worn as `display` has it.
    pub fn model(&self, file: u32, display: Option<&CreatureDisplay>) -> Result<Vec<u8>> {
        self.transcode(
            file,
            |k| display.and_then(|d| d.texture_for(k)),
            |id| match display {
                Some(d) => d.shows(id),
                None => id == 0 || id % 100 == 1,
            },
        )
    }

    /// The model in file `file` as a 1.12.1 MD20: `replace` fills its replaceable textures by
    /// type, `shows` picks its geosets.
    fn transcode(
        &self,
        file: u32,
        replace: impl Fn(u32) -> Option<u32>,
        shows: impl Fn(u16) -> bool,
    ) -> Result<Vec<u8>> {
        let chunks = md21::split(&self.casc.read_id(file)?)?;
        let m = model::parse(&chunks.md20)?;
        let skin = chunks
            .skins
            .first()
            .ok_or_else(|| Error::Format(format!("model {file} names no skin")))?;
        let skin = self.casc.read_id(*skin)?;
        let names: Vec<Option<String>> =
            transcode::texture_files(&m, &chunks.textures, replace, |p| self.casc.id_of(p))
                .into_iter()
                .map(|f| f.map(virtual_path))
                .collect();
        // The sequences kept out of the model, in their `.anim` files; one missing leaves its
        // sequence in the bind pose.
        let files: Vec<(u16, u16, Vec<u8>)> = chunks
            .anims
            .iter()
            .filter(|a| a.2 != 0)
            .filter_map(|&(id, variation, f)| Some((id, variation, self.casc.read_id(f).ok()?)))
            .collect();
        let animation = Animation {
            files: &files,
            fallbacks: &self.fallbacks,
        };
        transcode::to_vanilla(&chunks.md20, &skin, &names, shows, &animation)
    }
}

/// The item tables: the displays kept, the rest folded into what a look needs.
fn items<S: Source>(casc: &Casc<S>, dir: &Path) -> Result<Items> {
    let first_of = |t: Table, key: &str| -> HashMap<u32, u32> {
        let mut out = HashMap::new();
        for (&file, row) in &t.rows {
            if let Some(r) = row.int(key) {
                let e = out.entry(r as u32).or_insert(file);
                *e = (*e).min(file);
            }
        }
        out
    };
    let models = first_of(table(casc, dir, "ModelFileData")?, "ModelResourcesID");
    let textures = first_of(table(casc, dir, "TextureFileData")?, "MaterialResourcesID");
    let appearances = table(casc, dir, "ItemAppearance")?;
    let kinds = table(casc, dir, "Item")?;
    let mut worn = BTreeMap::new();
    for m in table(casc, dir, "ItemModifiedAppearance")?.rows.values() {
        let (Some(item), Some(app)) = (m.int("ItemID"), m.int("ItemAppearanceID")) else {
            continue;
        };
        let Some(display) = appearances
            .get(app as u32)
            .and_then(|a| a.int("ItemDisplayInfoID"))
            .filter(|&d| d > 0)
        else {
            continue;
        };
        let Some(kind) = kinds.get(item as u32) else {
            continue;
        };
        let class = kind.int("ClassID").unwrap_or(0) as u8;
        let subclass = kind.int("SubclassID").unwrap_or(0) as u8;
        let e = worn
            .entry(display as u32)
            .or_insert((class, subclass, item as u32));
        if (item as u32) < e.2 {
            *e = (class, subclass, item as u32);
        }
    }
    Ok(Items {
        displays: table(casc, dir, "ItemDisplayInfo")?,
        models,
        textures,
        worn,
    })
}

/// A Legion map the client may load: the directory its `Map.dbc` row names (`World\Maps\<ours>\`)
/// and Legion's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegionMap {
    pub ours: &'static str,
    pub legion: &'static str,
    /// Legion's `Map` id, which the client's row takes too: past every 1.12.1 map's.
    pub id: u32,
    /// Its name, as `Map`'s `MapName_lang` has it.
    pub name: &'static str,
}

/// The Legion maps the library answers: the Broken Isles (Legion's `troll raid`).
pub const LEGION_MAPS: &[LegionMap] = &[LegionMap {
    ours: "LegionBrokenIsles",
    legion: "troll raid",
    id: 1220,
    name: "Broken Isles",
}];

/// What a map path names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MapFile {
    Wdt,
    Tile(u32, u32),
}

/// A path under one of [`LEGION_MAPS`]' directories that [`Library::read`] answers: the tile table
/// or a tile.
fn map_path(path: &str) -> Option<(&'static LegionMap, MapFile)> {
    let rest = path.strip_prefix("world/maps/")?;
    let (dir, file) = rest.split_once('/')?;
    let map = LEGION_MAPS
        .iter()
        .find(|m| m.ours.eq_ignore_ascii_case(dir))?;
    let stem = file.strip_prefix(dir)?;
    if stem == ".wdt" {
        return Some((map, MapFile::Wdt));
    }
    let (x, y) = stem
        .strip_prefix('_')?
        .strip_suffix(".adt")?
        .split_once('_')?;
    Some((map, MapFile::Tile(x.parse().ok()?, y.parse().ok()?)))
}

/// A DB2 table from the install, typed by its definition in `dir`.
pub fn table<S: Source>(casc: &Casc<S>, dir: &Path, name: &str) -> Result<Table> {
    let raw = Raw::parse(&casc.read_path(&format!("DBFilesClient/{name}.db2"))?)?;
    let text = std::fs::read_to_string(dir.join(format!("{name}.dbd")))
        .map_err(|e| Error::Source(format!("{name}.dbd: {e}")))?;
    let def = Definition::parse(&text)?;
    let layout = def
        .layout(raw.layout_hash, BUILD)
        .ok_or_else(|| Error::Format(format!("{name}: no layout {:08X}", raw.layout_hash)))?;
    Ok(Table::new(&raw, layout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_legion_maps_tiles_and_table_are_its_paths_alone() {
        let dir = "world/maps/legionbrokenisles/legionbrokenisles";
        assert_eq!(map_path(&format!("{dir}.wdt")).unwrap().1, MapFile::Wdt);
        assert_eq!(
            map_path(&format!("{dir}_24_30.adt")).unwrap().1,
            MapFile::Tile(24, 30)
        );
        assert_eq!(map_path(&format!("{dir}.wdl")), None);
        assert_eq!(map_path(&format!("{dir}_24_30_tex0.adt")), None);
        assert_eq!(map_path("world/maps/azeroth/azeroth_32_48.adt"), None);
    }

    #[test]
    fn a_display_joins_the_catalog_past_vanillas_ids() {
        assert_eq!(display_model_path(61830), "Legion\\Display\\61830.mdx");
        const { assert!(DISPLAY_BASE > 100_000) };
    }
}
