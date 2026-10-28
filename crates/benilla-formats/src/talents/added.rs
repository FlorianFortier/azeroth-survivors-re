//! Tabs and talents added beside the DBCs' rows: a later client's classes (Legion's, through
//! `benilla-legion`), each specialization a tab of its own.

use super::{Talent, TalentCatalog, TalentTabInfo};

impl TalentCatalog {
    /// Add `tab` with its `talents`, after the tabs already there. A tab id already taken is
    /// refused, so the DBCs' rows are never replaced.
    pub fn add_tab(&mut self, tab: TalentTabInfo, talents: Vec<Talent>) -> bool {
        if self.tabs.iter().any(|t| t.id == tab.id) || self.by_tab.contains_key(&tab.id) {
            return false;
        }
        self.by_tab.insert(tab.id, talents);
        self.tabs.push(tab);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_added_tab_is_its_classs_and_keeps_the_dbcs() {
        let tab = |id, class_mask| TalentTabInfo {
            id,
            name: "Blood".into(),
            race_mask: u32::MAX,
            class_mask,
            background: "DeathKnightBlood".into(),
        };
        let talent = Talent {
            id: 7,
            tab: 100_250,
            row: 0,
            col: 1,
            ranks: [5, 5, 5, 0, 0],
            prereq_talent: 0,
            prereq_rank: 0,
            required_spell: 0,
            exceptional: false,
        };
        let mut c = TalentCatalog {
            tabs: Vec::new(),
            by_tab: Default::default(),
        };
        assert!(c.add_tab(tab(100_250, 1 << 5), vec![talent.clone()]));
        assert!(!c.add_tab(tab(100_250, 1 << 5), Vec::new()));
        assert_eq!(c.tabs_for_class(1, 6).len(), 1);
        assert!(c.tabs_for_class(1, 1).is_empty());
        assert_eq!(c.talents_in_tab(100_250), &[talent]);
    }
}
