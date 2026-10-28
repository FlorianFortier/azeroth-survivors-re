//! Legion's classes as far as a 1.12.1 client shows them: a spell's name and icon (`Spell`'s
//! `Name_lang`, `SpellMisc`'s `SpellIconFileDataID`), and each class's specializations
//! (`ChrSpecialization`) with their talents (`Talent`): seven tiers of three, a spec's own talent
//! taking the seat of the class's shared one.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use benilla_casc::{Casc, Source};

use crate::db2::Table;
use crate::library::table;
use crate::Result;

/// A spell as an icon slot shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegionSpell {
    pub id: u32,
    pub name: String,
    /// The icon's file data id, 0 for none.
    pub icon: u32,
}

/// One talent: its seat (tier 0 to 6, column 0 to 2) and the spell it teaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LegionTalent {
    pub id: u32,
    pub tier: u8,
    pub column: u8,
    pub spell: u32,
}

/// A specialization and its talents, by seat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegionSpec {
    pub id: u32,
    /// Its place among the class's specs.
    pub order: u8,
    pub name: String,
    /// Its icon's file data id.
    pub icon: u32,
    pub talents: Vec<LegionTalent>,
}

/// The tables, the spells' kept as names and icons alone.
pub(crate) struct Classes {
    names: HashMap<u32, String>,
    icons: HashMap<u32, u32>,
    specs: Table,
    talents: Table,
}

impl Classes {
    pub(crate) fn load<S: Source>(casc: &Casc<S>, dir: &Path) -> Result<Self> {
        let names = table(casc, dir, "Spell")?
            .rows
            .into_iter()
            .filter_map(|(id, r)| Some((id, r.text("Name_lang")?.to_string())))
            .collect();
        // The first row of the base difficulty names a spell's icon.
        let mut icons = HashMap::new();
        for r in table(casc, dir, "SpellMisc")?.rows.values() {
            let (Some(spell), Some(icon)) = (r.int("SpellID"), r.int("SpellIconFileDataID")) else {
                continue;
            };
            if r.int("DifficultyID").unwrap_or(0) == 0 && icon > 0 {
                icons.entry(spell as u32).or_insert(icon as u32);
            }
        }
        Ok(Self {
            names,
            icons,
            specs: table(casc, dir, "ChrSpecialization")?,
            talents: table(casc, dir, "Talent")?,
        })
    }

    pub(crate) fn spell(&self, id: u32) -> Option<LegionSpell> {
        Some(LegionSpell {
            id,
            name: self.names.get(&id)?.clone(),
            icon: self.icons.get(&id).copied().unwrap_or(0),
        })
    }

    /// Class `class`'s specs in their order, each with its talents.
    pub(crate) fn specs(&self, class: u8) -> Vec<LegionSpec> {
        let class = i64::from(class);
        let mut specs: Vec<LegionSpec> = self
            .specs
            .rows
            .values()
            .filter(|r| r.int("ClassID") == Some(class))
            .map(|r| LegionSpec {
                id: r.id,
                order: r.int("OrderIndex").unwrap_or(0) as u8,
                name: r.text("Name_lang").unwrap_or_default().to_string(),
                icon: r.int("SpellIconFileID").unwrap_or(0) as u32,
                talents: self.talents(class, r.id),
            })
            .filter(|s| !s.talents.is_empty())
            .collect();
        specs.sort_by_key(|s| s.order);
        specs
    }

    /// Spec `spec`'s talents in seat order: its own, else the class's shared one in that seat.
    fn talents(&self, class: i64, spec: u32) -> Vec<LegionTalent> {
        let mut seats: BTreeMap<(u8, u8), (bool, LegionTalent)> = BTreeMap::new();
        for r in self.talents.rows.values() {
            let own = r.int("SpecID") == Some(i64::from(spec));
            let shared = r.int("SpecID") == Some(0) && r.int("ClassID") == Some(class);
            if !own && !shared {
                continue;
            }
            let t = LegionTalent {
                id: r.id,
                tier: r.int("TierID").unwrap_or(0) as u8,
                column: r.int("ColumnIndex").unwrap_or(0) as u8,
                spell: r.int("SpellID").unwrap_or(0) as u32,
            };
            if t.spell == 0 {
                continue;
            }
            let seat = (t.tier, t.column);
            if seats.get(&seat).is_none_or(|(was_own, _)| own && !was_own) {
                seats.insert(seat, (own, t));
            }
        }
        seats.into_values().map(|(_, t)| t).collect()
    }
}
