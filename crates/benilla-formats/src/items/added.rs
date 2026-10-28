//! Item displays added to the catalog beside the DBC's rows: a later client's looks (Legion's,
//! through `benilla-legion`), each with a model basename the asset reader answers.

use super::{ItemDisplay, ItemDisplayCatalog};

impl ItemDisplayCatalog {
    /// Add display `id`. A row already there is kept: the DBC's ids are never taken.
    pub fn add_display(&mut self, id: u32, display: ItemDisplay) -> bool {
        if self.displays.contains_key(&id) {
            return false;
        }
        self.displays.insert(id, display);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn an_added_display_never_takes_a_dbc_id() {
        let mut c = ItemDisplayCatalog::from_displays(HashMap::from([(5, ItemDisplay::default())]));
        let look = ItemDisplay {
            model: [Some("legion_item_45150_0.m2".into()), None],
            ..ItemDisplay::default()
        };
        assert!(!c.add_display(5, look.clone()));
        assert!(c.get(5).unwrap().model[0].is_none());
        assert!(c.add_display(0xf000, look));
        assert_eq!(
            c.get(0xf000).unwrap().model[0].as_deref(),
            Some("legion_item_45150_0.m2")
        );
    }
}
