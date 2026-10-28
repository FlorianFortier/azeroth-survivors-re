//! Hair and facial-hair geosets added beside the DBCs' rows, for a later client's races
//! (Legion's, through `benilla-legion`).

use super::CharacterGeosets;

impl CharacterGeosets {
    /// Add `(race, sex, hair style)`'s hair geoset; a row already there is kept.
    pub fn add_hair(&mut self, key: (u8, u8, u8), geoset: u32) -> bool {
        if self.hair.contains_key(&key) {
            return false;
        }
        self.hair.insert(key, geoset);
        true
    }

    /// Add `(race, sex, facial hair)`'s three geosets, groups 1, 3 and 2 in that order; a row
    /// already there is kept.
    pub fn add_facial(&mut self, key: (u8, u8, u8), geosets: [u32; 3]) -> bool {
        if self.facial.contains_key(&key) {
            return false;
        }
        self.facial.insert(key, geosets);
        true
    }
}
