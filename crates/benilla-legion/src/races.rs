//! Legion's playable races in their pre-HD bodies, as 1.12.1's character tables hold a race: the
//! body displays (`ChrRaces`' `MaleDisplayID`/`FemaleDisplayID`, not the `HighRes…` ones), the
//! skin, face, facial hair, hair and underwear textures (`CharSections`' sections 0 to 4, the old
//! layout; 5 to 9 are the HD bodies'), the hair geosets (`CharHairGeosets`) and the facial hair's
//! (`CharacterFacialHairStyles`).
//!
//! Legion ships the old layout's textures at twice 1.12.1's size (512² for a 256² atlas): the
//! client reads them through [`half_blp`], their first mip level dropped, so they land on the
//! compositor's 1.12.1 tiles as they are.

use std::collections::BTreeMap;
use std::path::Path;

use benilla_casc::{Casc, Source};

use crate::db2::Table;
use crate::library::table;
use crate::Result;

/// One `CharSections` row of the old layout: `(sex, section, variation, colour)` and its three
/// textures' file data ids (0 for none).
pub type Section = ((u8, u8, u8, u8), [u32; 3]);

/// A race as 1.12.1's tables hold one.
#[derive(Clone, Debug, PartialEq)]
pub struct LegionRace {
    pub id: u8,
    /// `ChrRaces`' `ClientFileString`: `BloodElf`, `Draenei`.
    pub file: String,
    /// Whether it is the Horde's (`ChrRaces`' `Alliance` 1).
    pub horde: bool,
    /// The male and female body displays (Legion `CreatureDisplayInfo` ids) and their models'
    /// file data ids.
    pub displays: [u32; 2],
    pub models: [u32; 2],
    pub sections: Vec<Section>,
    /// `(sex, hair style)` to its geoset.
    pub hair: Vec<((u8, u8), u32)>,
    /// `(sex, facial hair)` to its three geosets, groups 1, 3 and 2.
    pub facial: Vec<((u8, u8), [u32; 3])>,
}

/// The race tables.
pub(crate) struct Races {
    races: Table,
    sections: Table,
    hair: Table,
    facial: Table,
}

/// `CharSections` flags a playable body's rows never carry: alternates (0x4), death knights'
/// (0x8), the demon hunter's and the HD bodies' extras (0x20), unused rows (0x100).
const SKIPPED_FLAGS: i64 = 0x4 | 0x8 | 0x20 | 0x100;

impl Races {
    pub(crate) fn load<S: Source>(casc: &Casc<S>, dir: &Path) -> Result<Self> {
        Ok(Self {
            races: table(casc, dir, "ChrRaces")?,
            sections: table(casc, dir, "CharSections")?,
            hair: table(casc, dir, "CharHairGeosets")?,
            facial: table(casc, dir, "CharacterFacialHairStyles")?,
        })
    }

    /// Race `id`, its textures resolved by `texture` (a material resource to its file) and its
    /// body displays' models by `model`.
    pub(crate) fn race(
        &self,
        id: u8,
        texture: impl Fn(u32) -> u32,
        model: impl Fn(u32) -> u32,
    ) -> Option<LegionRace> {
        let row = self.races.get(u32::from(id))?;
        let displays = [
            row.int("MaleDisplayID")? as u32,
            row.int("FemaleDisplayID")? as u32,
        ];
        let race = Some(i64::from(id));
        let mut sections: BTreeMap<(u8, u8, u8, u8), (i64, [u32; 3])> = BTreeMap::new();
        for r in self
            .sections
            .rows
            .values()
            .filter(|r| r.int("RaceID") == race)
        {
            let flags = r.int("Flags").unwrap_or(0);
            let base = r.int("BaseSection").unwrap_or(-1);
            if flags & SKIPPED_FLAGS != 0 || !(0..=4).contains(&base) {
                continue;
            }
            let key = (
                r.int("SexID").unwrap_or(0) as u8,
                base as u8,
                r.int("VariationIndex").unwrap_or(0) as u8,
                r.int("ColorIndex").unwrap_or(0) as u8,
            );
            let mats = r.ints("MaterialResourcesID");
            let files = [0, 1, 2].map(|i| {
                mats.get(i)
                    .filter(|&&m| m > 0)
                    .map_or(0, |&m| texture(m as u32))
            });
            // The playable row (flag 0x10) first, else the fewest flags.
            let rank = if flags & 0x10 != 0 { -1 } else { flags };
            if sections.get(&key).is_none_or(|(r, _)| rank < *r) {
                sections.insert(key, (rank, files));
            }
        }
        let mut hair: BTreeMap<(u8, u8), (u32, u32)> = BTreeMap::new();
        for r in self.hair.rows.values().filter(|r| r.int("RaceID") == race) {
            let plain = r.int("VariationType") == Some(3)
                && r.int("GeosetType") == Some(0)
                && r.int("CustomGeoFileDataID").unwrap_or(0) == 0
                && r.int("HdCustomGeoFileDataID").unwrap_or(0) == 0;
            if !plain {
                continue;
            }
            let key = (
                r.int("SexID").unwrap_or(0) as u8,
                r.int("VariationID").unwrap_or(0) as u8,
            );
            let geoset = r.int("GeosetID").unwrap_or(0) as u32;
            if hair.get(&key).is_none_or(|(rid, _)| r.id < *rid) {
                hair.insert(key, (r.id, geoset));
            }
        }
        let facial = self
            .facial
            .rows
            .values()
            .filter(|r| r.int("RaceID") == race)
            .map(|r| {
                let g = r.ints("Geoset");
                let at = |i: usize| g.get(i).copied().unwrap_or(0).max(0) as u32;
                (
                    (
                        r.int("SexID").unwrap_or(0) as u8,
                        r.int("VariationID").unwrap_or(0) as u8,
                    ),
                    [at(0), at(1), at(2)],
                )
            })
            .collect::<BTreeMap<_, _>>();
        Some(LegionRace {
            id,
            file: row.text("ClientFileString")?.to_string(),
            horde: row.int("Alliance") == Some(1),
            displays,
            models: displays.map(&model),
            sections: sections.into_iter().map(|(k, (_, f))| (k, f)).collect(),
            hair: hair.into_iter().map(|(k, (_, g))| (k, g)).collect(),
            facial: facial.into_iter().collect(),
        })
    }
}

/// A BLP2 with its first mip level dropped: the same art at half the size. One with a single
/// level, or not a BLP2, is returned as it is.
pub fn half_blp(b: &[u8]) -> Vec<u8> {
    let u32_at = |at: usize| {
        b.get(at..at + 4)
            .map(|s| u32::from_le_bytes(s.try_into().unwrap()))
    };
    if b.get(0..4) != Some(b"BLP2") || b.get(11) == Some(&0) {
        return b.to_vec();
    }
    let (Some(w), Some(h)) = (u32_at(12), u32_at(16)) else {
        return b.to_vec();
    };
    let offsets: Vec<u32> = (0..16).filter_map(|i| u32_at(20 + i * 4)).collect();
    let sizes: Vec<u32> = (0..16).filter_map(|i| u32_at(84 + i * 4)).collect();
    if offsets.len() < 16 || sizes.len() < 16 || offsets[1] == 0 || sizes[1] == 0 || w < 2 || h < 2
    {
        return b.to_vec();
    }
    let mut out = b.to_vec();
    out[12..16].copy_from_slice(&(w / 2).to_le_bytes());
    out[16..20].copy_from_slice(&(h / 2).to_le_bytes());
    for i in 0..16 {
        let (o, s) = if i < 15 {
            (offsets[i + 1], sizes[i + 1])
        } else {
            (0, 0)
        };
        out[20 + i * 4..24 + i * 4].copy_from_slice(&o.to_le_bytes());
        out[84 + i * 4..88 + i * 4].copy_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4×4 BLP2 of raw BGRA, three levels.
    fn blp() -> Vec<u8> {
        let mut b = vec![0u8; 148 + 1024];
        b[0..4].copy_from_slice(b"BLP2");
        b[4..8].copy_from_slice(&1u32.to_le_bytes());
        b[8] = 3;
        b[9] = 8;
        b[11] = 1;
        b[12..16].copy_from_slice(&4u32.to_le_bytes());
        b[16..20].copy_from_slice(&4u32.to_le_bytes());
        let mut at = b.len() as u32;
        for (i, size) in [64u32, 16, 4].into_iter().enumerate() {
            b[20 + i * 4..24 + i * 4].copy_from_slice(&at.to_le_bytes());
            b[84 + i * 4..88 + i * 4].copy_from_slice(&size.to_le_bytes());
            at += size;
        }
        b.extend((0..84u8).map(|x| x));
        b
    }

    #[test]
    fn a_halved_blp_starts_at_its_second_level() {
        let b = blp();
        let h = half_blp(&b);
        let u = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        assert_eq!((u(&h, 12), u(&h, 16)), (2, 2));
        assert_eq!((u(&h, 20), u(&h, 84)), (u(&b, 24), 16));
        assert_eq!((u(&h, 24), u(&h, 88)), (u(&b, 28), 4));
        assert_eq!(u(&h, 28), 0);
        // One level only: kept as it is.
        let mut single = b.clone();
        single[11] = 0;
        assert_eq!(half_blp(&single), single);
    }
}
