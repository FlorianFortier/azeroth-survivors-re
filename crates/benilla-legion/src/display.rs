//! A creature's look in Legion, as `CreatureDisplayInfo` and `CreatureModelData` give it: the model
//! file, the three replaceable textures (a model's texture records of types 11 to 13), the geosets
//! shown and the scale.

use crate::db2::Table;

#[derive(Clone, Debug, PartialEq)]
pub struct CreatureDisplay {
    pub id: u32,
    /// The `CreatureModelData` row.
    pub model: u32,
    /// The model's file data id.
    pub model_file: u32,
    /// The textures for texture types 11, 12 and 13; 0 for none.
    pub textures: [u32; 3],
    /// `CreatureGeosetData`: four bits a geoset group, groups 1 to 8.
    pub geosets: u32,
    pub scale: f32,
}

impl CreatureDisplay {
    /// The texture a texture record of `kind` takes, if it is a replaceable one this display fills.
    pub fn texture_for(&self, kind: u32) -> Option<u32> {
        let i = kind.checked_sub(11)? as usize;
        self.textures.get(i).copied().filter(|&t| t != 0)
    }

    /// Whether a skin section with geoset id `id` (`group * 100 + variant`) shows: the body
    /// (group 0) always; groups 1 to 8 the variant their four bits pick, the first when they pick
    /// none; every later group its first variant.
    pub fn shows(&self, id: u16) -> bool {
        let (group, variant) = (u32::from(id / 100), u32::from(id % 100));
        match group {
            0 => true,
            1..=8 => {
                let picked = (self.geosets >> (4 * (group - 1))) & 0xf;
                variant == picked.max(1)
            }
            _ => variant == 1,
        }
    }
}

/// One display by its id.
pub fn creature_display(info: &Table, models: &Table, id: u32) -> Option<CreatureDisplay> {
    let row = info.get(id)?;
    let model = row.int("ModelID")? as u32;
    let model_file = models.get(model)?.int("FileDataID")? as u32;
    let tex = row.ints("TextureVariationFileDataID");
    let at = |i: usize| tex.get(i).copied().unwrap_or(0) as u32;
    Some(CreatureDisplay {
        id,
        model,
        model_file,
        textures: [at(0), at(1), at(2)],
        geosets: row.int("CreatureGeosetData").unwrap_or(0) as u32,
        scale: row.float("CreatureModelScale").unwrap_or(1.0),
    })
}

/// Every display of the model in file `model_file`, by id.
pub fn displays_of_model(info: &Table, models: &Table, model_file: u32) -> Vec<CreatureDisplay> {
    let model_rows: Vec<u32> = models
        .rows
        .values()
        .filter(|r| r.int("FileDataID") == Some(i64::from(model_file)))
        .map(|r| r.id)
        .collect();
    info.rows
        .values()
        .filter(|r| {
            r.int("ModelID")
                .is_some_and(|m| model_rows.contains(&(m as u32)))
        })
        .filter_map(|r| creature_display(info, models, r.id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geosets_follow_the_displays_four_bits() {
        let d = CreatureDisplay {
            id: 1,
            model: 1,
            model_file: 1,
            textures: [5, 0, 7],
            // Group 1 variant 3, group 2 none (its first).
            geosets: 0x03,
            scale: 1.0,
        };
        assert!(d.shows(0) && d.shows(5));
        assert!(d.shows(103) && !d.shows(101));
        assert!(d.shows(201) && !d.shows(202));
        assert!(d.shows(1301) && !d.shows(1302));
        assert_eq!(
            [d.texture_for(11), d.texture_for(12), d.texture_for(13)],
            [Some(5), None, Some(7)]
        );
        assert_eq!(d.texture_for(0), None);
    }
}
