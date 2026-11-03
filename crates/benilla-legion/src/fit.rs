//! A character body within 1.12.1's reach. A skin profile's sections start their triangles at a
//! 16-bit index, so the triangles a body keeps must stay within [`MAX_INDICES`]; Legion's later
//! bodies pass it with every geoset (a Pandaren's 72,000 to 88,000 indices, a Blood Elf's HD body
//! 120,000, its seventeen hair styles 60,000 of them).
//!
//! [`body_geosets`] keeps the groups 1.12.1 draws ([`GROUPS`]), the body itself (geoset 0),
//! then the hair styles' geosets in their styles' order while they fit; a style left out is one
//! the race's tables no longer offer (`Library::race`).

use std::collections::BTreeSet;

use crate::skin::Skin;

/// The geoset groups a body keeps: 1.12.1's, but the robe's skirt (13) and the cape (15).
pub const GROUPS: [u16; 13] = [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11, 12, 17];

/// The triangle indices a 1.12.1 skin profile reaches.
pub const MAX_INDICES: u32 = 65_535;

/// The geosets of `skin` a body keeps: every one of [`GROUPS`] but the hair's (group 0's
/// variants), then each of `hair`'s geosets (its styles', in their order) that still fits.
pub fn body_geosets(skin: &Skin, hair: &[u32]) -> BTreeSet<u16> {
    let cost = |id: u16| -> u32 {
        skin.sections
            .iter()
            .filter(|s| s.id == id)
            .map(|s| u32::from(s.index_count))
            .sum()
    };
    let mut kept: BTreeSet<u16> = skin
        .sections
        .iter()
        .map(|s| s.id)
        .filter(|&id| id == 0 || (id >= 100 && GROUPS.contains(&(id / 100))))
        .collect();
    let mut total: u32 = kept.iter().map(|&id| cost(id)).sum();
    for &g in hair {
        let Ok(id) = u16::try_from(g) else {
            continue;
        };
        if id == 0 || id >= 100 || kept.contains(&id) {
            continue;
        }
        let c = cost(id);
        if total + c <= MAX_INDICES {
            kept.insert(id);
            total += c;
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skin::Section;

    fn section(id: u16, index_count: u16) -> Section {
        Section {
            id,
            vertex_start: 0,
            vertex_count: 0,
            index_start: 0,
            index_count,
        }
    }

    #[test]
    fn a_body_keeps_its_groups_then_its_hair_while_it_fits() {
        let skin = Skin {
            vertices: Vec::new(),
            indices: Vec::new(),
            sections: vec![
                section(0, 20_000),
                section(0, 5_000),
                section(1, 15_000),
                section(2, 16_000),
                section(3, 15_000),
                section(4, 1_000),
                section(401, 10_000),
                section(1301, 9_000),
                section(1501, 9_000),
                section(2201, 9_000),
            ],
            batches: Vec::new(),
        };
        // The body, the gloves; the styles in order, 2 past the budget, 4 small enough after it.
        let kept = body_geosets(&skin, &[1, 1, 2, 0, 4, 3]);
        assert_eq!(kept, BTreeSet::from([0, 1, 4, 401]));
        let total: u32 = skin
            .sections
            .iter()
            .filter(|s| kept.contains(&s.id))
            .map(|s| u32::from(s.index_count))
            .sum();
        assert!(total <= MAX_INDICES);
    }
}
