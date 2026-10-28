//! Body sections added beside the DBC's rows: a later client's races (Legion's, through
//! `benilla-legion`), their textures named by paths the chain answers beside its archives.

use super::CharSections;

impl CharSections {
    /// Add the textures of `(race, sex, section, variation, colour)`. A row already there is kept:
    /// the DBC's are never replaced.
    pub fn add_section(&mut self, key: (u8, u8, u8, u8, u8), textures: [String; 3]) -> bool {
        if self.sections.contains_key(&key) {
            return false;
        }
        self.sections.insert(key, textures);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn an_added_race_wears_its_skin() {
        let mut s = CharSections {
            sections: HashMap::new(),
        };
        let skin = |p: &str| [p.to_string(), String::new(), String::new()];
        assert!(s.add_section((10, 0, 0, 0, 2), skin("Legion\\\\Half\\\\117385.blp")));
        assert!(!s.add_section((10, 0, 0, 0, 2), skin("other.blp")));
        assert_eq!(
            s.skin_texture(10, 0, 2),
            Some("Legion\\\\Half\\\\117385.blp")
        );
        assert_eq!(s.skin_extra_texture(10, 0, 2), None);
    }
}
