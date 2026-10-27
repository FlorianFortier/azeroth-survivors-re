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
//! The paths come as the asset reader holds them: lowercase, `/` between folders. A creature
//! display joins the client's catalog as [`DISPLAY_BASE`] plus its Legion id, its model named
//! [`display_model_path`].

use std::path::Path;

use benilla_casc::{Casc, Source};

use crate::db2::{Raw, Table};
use crate::dbd::Definition;
use crate::display::{self, CreatureDisplay};
use crate::transcode::{self, virtual_path};
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

/// A source any thread may read: an install on disk or a remote copy.
pub type AnySource = Box<dyn Source + Send + Sync>;

/// The creature display tables.
struct Displays {
    info: Table,
    models: Table,
}

/// An open install and, with the definitions, its creature displays.
pub struct Library {
    casc: Casc<AnySource>,
    displays: Option<Displays>,
}

impl Library {
    /// Open the install; `dbd` is a folder of WoWDBDefs' `.dbd` definitions, which the creature
    /// displays need.
    pub fn open(source: AnySource, dbd: Option<&Path>) -> Result<Self> {
        let casc = Casc::open(source)?;
        let displays = match dbd {
            Some(dir) => Some(Displays {
                info: table(&casc, dir, "CreatureDisplayInfo")?,
                models: table(&casc, dir, "CreatureModelData")?,
            }),
            None => None,
        };
        Ok(Self { casc, displays })
    }

    /// The file at `path`, `None` when it is not a Legion path.
    pub fn read(&self, path: &str) -> Option<Result<Vec<u8>>> {
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
        } else if let Some(file) = number(rest, ".blp") {
            self.casc.read_id(file).map_err(Error::from)
        } else {
            Err(Error::Format(format!("no Legion file at {path}")))
        })
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

    /// The model in file `file` as a 1.12.1 MD20, worn as `display` has it.
    pub fn model(&self, file: u32, display: Option<&CreatureDisplay>) -> Result<Vec<u8>> {
        let chunks = md21::split(&self.casc.read_id(file)?)?;
        let m = model::parse(&chunks.md20)?;
        let skin = chunks
            .skins
            .first()
            .ok_or_else(|| Error::Format(format!("model {file} names no skin")))?;
        let skin = self.casc.read_id(*skin)?;
        let names: Vec<Option<String>> =
            transcode::texture_files(&m, &chunks.textures, display, |p| self.casc.id_of(p))
                .into_iter()
                .map(|f| f.map(virtual_path))
                .collect();
        let shows = |id: u16| match display {
            Some(d) => d.shows(id),
            None => id == 0 || id % 100 == 1,
        };
        transcode::to_vanilla(&chunks.md20, &skin, &names, shows)
    }
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
    fn a_display_joins_the_catalog_past_vanillas_ids() {
        assert_eq!(display_model_path(61830), "Legion\\Display\\61830.mdx");
        assert!(DISPLAY_BASE > 100_000);
    }
}
