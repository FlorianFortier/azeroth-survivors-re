//! Displays added to the catalog beside the DBCs' rows: a later client's creatures (Legion's,
//! through `benilla-legion`), each a model path the asset reader answers, a scale and a collision
//! height, no skins (the model names its textures) and no humanoid appearance.

use super::{CreatureCatalog, DisplayRow, ModelRow};

impl CreatureCatalog {
    /// Add display `id`, its model at `model_path` (`.mdx`, as the DBCs name theirs) drawn at
    /// `scale`. A row already there is kept: the DBCs' ids are never taken.
    pub fn add_display(
        &mut self,
        id: u32,
        model_path: &str,
        scale: f32,
        collision_height: f32,
    ) -> bool {
        if self.display.contains_key(&id) {
            return false;
        }
        // A model row of its own, keyed past every DBC row by the display's id.
        let model_id = id;
        self.models.entry(model_id).or_insert_with(|| ModelRow {
            path: model_path.to_string(),
            scale: 1.0,
            flags: 0,
            size_class: -1,
            blood: -1,
            footprint_texture: -1,
            footprint_length: 0.0,
            footprint_width: 0.0,
            collision_height,
            foley_material: 0,
            footstep_shake: 0,
            death_thud_shake: 0,
        });
        self.display.insert(
            id,
            DisplayRow {
                model_id,
                extended_id: 0,
                scale,
                textures: [None, None, None],
                blood_level: 0,
                size_class: -1,
                model_alpha: 255,
            },
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_added_display_resolves_like_a_dbc_one() {
        let mut c = CreatureCatalog::default();
        assert!(c.add_display(1_061_830, "Legion\\Display\\61830.mdx", 2.5, 3.0));
        assert!(!c.add_display(1_061_830, "Other.mdx", 1.0, 0.0));
        let m = c.model(1_061_830).unwrap();
        assert_eq!(m.model_path, "Legion\\Display\\61830.mdx");
        assert_eq!((m.scale, m.collision_height), (2.5, 3.0));
        assert!(m.textures.iter().all(Option::is_none) && m.npc_appearance.is_none());
        assert_eq!(c.display_scale(1_061_830), Some(2.5));
    }
}
