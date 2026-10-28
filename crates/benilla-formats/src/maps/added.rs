//! Maps added beside the DBC's rows: a later client's (Legion's, through `benilla-legion`), each
//! a directory whose files the patch chain's extra files answer.

use super::MapCatalog;

impl MapCatalog {
    /// Add map `id`, an open world (instance type 0) in `directory`, shown as `name`. An id
    /// already taken is refused, so the DBC's rows are never replaced.
    pub fn add_map(&mut self, id: u32, directory: &str, name: &str) -> bool {
        if self.dirs.contains_key(&id) {
            return false;
        }
        self.dirs.insert(id, directory.to_string());
        self.names.insert(id, name.to_string());
        self.instance_types.insert(id, 0);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_added_map_is_an_open_world_and_keeps_the_dbcs() {
        let mut c = MapCatalog {
            dirs: [(0, "Azeroth".to_string())].into(),
            names: Default::default(),
            loading_screens: Default::default(),
            instance_types: Default::default(),
            battleground: Default::default(),
        };
        assert!(c.add_map(1220, "LegionBrokenIsles", "Broken Isles"));
        assert!(!c.add_map(0, "Elsewhere", "Elsewhere"));
        assert_eq!(c.directory(1220), Some("LegionBrokenIsles"));
        assert_eq!(c.directory(0), Some("Azeroth"));
        assert_eq!(c.name(1220), Some("Broken Isles"));
        assert_eq!(c.instance_type(1220), Some(0));
    }
}
