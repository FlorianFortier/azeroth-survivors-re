//! Races added beside the DBCs' rows, for a later client's races (Legion's, through
//! `benilla-legion`): their body displays, the classes they may be and their dial ranges.

use super::{CharCreateCatalog, DialRanges};

impl CharCreateCatalog {
    /// Add race `race`: its male and female body displays, its `ChrRaces` file string, the
    /// classes it may be, and its dial ranges by sex. A race already there is kept.
    pub fn add_race(
        &mut self,
        race: u8,
        displays: (u32, u32),
        file: &str,
        classes: &[u8],
        ranges: [Option<DialRanges>; 2],
    ) -> bool {
        if self.displays.contains_key(&race) {
            return false;
        }
        self.displays.insert(race, displays);
        self.files.insert(race, file.to_string());
        self.custom_tokens.insert(
            race,
            (
                ["NORMAL".to_string(), "NORMAL".to_string()],
                "NORMAL".to_string(),
            ),
        );
        for &class in classes {
            self.combos.insert((race, class));
        }
        for (sex, r) in ranges.into_iter().enumerate() {
            if let Some(r) = r {
                self.ranges.insert((race, sex as u8), r);
            }
        }
        true
    }
}
