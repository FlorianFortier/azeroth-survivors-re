//! The player's profile: everything that outlives a run. Today the hero picked at the menu and
//! the record of every run, per zone and overall; the meta-progression (gold, collection, gear,
//! talents, achievements) joins it as it lands, each as a new `#[serde(default)]` field, so an
//! older profile keeps loading.
//!
//! It lives in `benilla-config/survivors-profile.toml` (the page's `localStorage` in the browser,
//! through [`crate::local_state`]). Classes and zones are named, not numbered, so reordering the
//! tables never moves a record onto another class or zone.
//!
//! A profile that does not parse is kept beside as `.bad` and play starts from a fresh one; a
//! profile written by a newer game is read but never overwritten, so going back a version loses
//! nothing.

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::data;
use super::{Phase, Run, Selection, Session};

/// Bumped when a field changes meaning; a new field with a default does not need it.
pub(super) const PROFILE_VERSION: u32 = 1;

const FILE: &str = "survivors-profile.toml";
/// The pick file the profile replaced, read once to carry the last hero over.
const OLD_PICK_FILE: &str = "survivors-pick.txt";
/// Seconds a change waits before it is written, so a burst of clicks is one write.
const SAVE_DELAY: f32 = 0.5;

/// The hero picked at the menu.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub(super) struct HeroPick {
    /// [`data::ClassDef::name`]; empty for none yet.
    pub class: String,
    /// `ChrRaces.dbc` id and sex (0 male, 1 female) of the look.
    pub race: u8,
    pub sex: u8,
    /// [`data::MapDef::name`] of the zone.
    pub zone: String,
}

/// One zone's record.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub(super) struct ZoneRecord {
    pub runs: u32,
    pub wins: u32,
    /// The longest a run lasted here, in seconds.
    pub best_time: f32,
    pub best_level: u32,
    pub best_kills: u32,
}

/// Every run, added up.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub(super) struct Totals {
    pub runs: u32,
    pub wins: u32,
    pub kills: u64,
    pub damage_done: f64,
    pub best_level: u32,
    /// Seconds spent in runs.
    pub time_played: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub(super) struct Profile {
    pub version: u32,
    pub hero: HeroPick,
    pub totals: Totals,
    /// By [`data::MapDef::name`].
    pub zones: BTreeMap<String, ZoneRecord>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: PROFILE_VERSION,
            hero: HeroPick::default(),
            totals: Totals::default(),
            zones: BTreeMap::new(),
        }
    }
}

/// How a run ended, for the record.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RunResult {
    pub zone: String,
    pub won: bool,
    pub secs: f32,
    pub level: u32,
    pub kills: u32,
    pub damage_done: f64,
}

impl Profile {
    /// Read a profile file's text: `Err` for text that is not a profile at all. Missing fields
    /// take their defaults and unknown ones are skipped, so any version's file loads.
    pub fn parse(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    pub fn to_text(&self) -> String {
        toml::to_string_pretty(self).unwrap_or_default()
    }

    /// A newer game wrote this: read it, never write over it.
    pub fn from_newer_game(&self) -> bool {
        self.version > PROFILE_VERSION
    }

    /// Add a finished run.
    pub fn record(&mut self, r: &RunResult) {
        let t = &mut self.totals;
        t.runs += 1;
        t.wins += u32::from(r.won);
        t.kills += u64::from(r.kills);
        t.damage_done += r.damage_done.max(0.0);
        t.best_level = t.best_level.max(r.level);
        t.time_played += f64::from(r.secs.max(0.0));
        let z = self.zones.entry(r.zone.clone()).or_default();
        z.runs += 1;
        z.wins += u32::from(r.won);
        z.best_time = z.best_time.max(r.secs);
        z.best_level = z.best_level.max(r.level);
        z.best_kills = z.best_kills.max(r.kills);
    }

    /// What the profile brings into a run, for the hero picked: the bonuses that hold all run
    /// and the effects that fire on their own. The collection, talents, racials and gear each add
    /// theirs here as they land; today the profile brings nothing yet.
    pub fn run_bonuses(&self) -> (Vec<data::Bonus>, Vec<&'static data::ProcDef>) {
        (Vec::new(), Vec::new())
    }

    /// The pick file the profile replaced: `class race sex zone`, all indices.
    pub fn from_old_pick(text: &str) -> Option<Self> {
        let v: Vec<usize> = text
            .split_whitespace()
            .filter_map(|t| t.parse().ok())
            .collect();
        let [class, race, sex, zone] = v[..] else {
            return None;
        };
        let class = data::CLASSES.get(class)?;
        let zone = data::MAPS.get(zone)?;
        Some(Self {
            hero: HeroPick {
                class: class.name.to_string(),
                race: u8::try_from(race).ok()?,
                sex: u8::try_from(sex).ok()?,
                zone: zone.name.to_string(),
            },
            ..Self::default()
        })
    }
}

/// The class and zone of a pick, as indices into today's tables; a name no table has any more is
/// `None`.
pub(super) fn pick_indices(hero: &HeroPick) -> (Option<usize>, Option<usize>) {
    (
        data::CLASSES.iter().position(|c| c.name == hero.class),
        data::MAPS.iter().position(|m| m.name == hero.zone),
    )
}

/// The profile, where it lives, and whether it has changes to write.
#[derive(Resource)]
pub(super) struct ProfileStore {
    pub profile: Profile,
    path: Option<PathBuf>,
    dirty: bool,
    wait: f32,
    /// Never written: a newer game's profile, or no state folder (a capture, a test).
    read_only: bool,
}

impl ProfileStore {
    /// In memory only.
    pub fn ephemeral(profile: Profile) -> Self {
        Self {
            profile,
            path: None,
            dirty: false,
            wait: 0.0,
            read_only: true,
        }
    }

    /// Change the profile; it is written shortly after.
    pub fn edit(&mut self, f: impl FnOnce(&mut Profile)) {
        let before = self.profile.clone();
        f(&mut self.profile);
        if self.profile != before {
            self.dirty = true;
            self.wait = 0.0;
        }
    }

    /// Read the profile from the state folder: the old pick file the first time, a fresh profile
    /// when there is neither, the broken file set aside when it does not parse.
    pub fn load() -> Self {
        let Some(home) = crate::local_state::home() else {
            return Self::ephemeral(Profile::default());
        };
        let path = home.join(FILE);
        let (profile, dirty) = match crate::local_state::read_to_string(&path) {
            Ok(text) => match Profile::parse(&text) {
                Ok(p) => (p, false),
                Err(e) => {
                    warn!(
                        "survivors: profile unreadable ({e}); kept as {FILE}.bad, starting fresh"
                    );
                    let _ =
                        crate::local_state::write_atomic(&home.join(format!("{FILE}.bad")), &text);
                    (Profile::default(), true)
                }
            },
            Err(_) => {
                let old = crate::local_state::read_to_string(&home.join(OLD_PICK_FILE))
                    .ok()
                    .and_then(|t| Profile::from_old_pick(&t));
                match old {
                    Some(p) => (p, true),
                    None => (Profile::default(), false),
                }
            }
        };
        let read_only = profile.from_newer_game();
        if read_only {
            warn!(
                "survivors: the profile is from a newer game (version {}); it will not be saved",
                profile.version
            );
        }
        Self {
            profile,
            path: Some(path),
            dirty: dirty && !read_only,
            wait: 0.0,
            read_only,
        }
    }

    fn save_now(&mut self) {
        self.dirty = false;
        if self.read_only {
            return;
        }
        let Some(path) = &self.path else { return };
        self.profile.version = PROFILE_VERSION;
        if let Err(e) = crate::local_state::write_atomic(path, &self.profile.to_text()) {
            warn!("survivors: could not save the profile: {e}");
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(ProfileStore::load())
        .add_systems(Update, save_when_settled)
        .add_systems(OnEnter(Phase::GameOver), record_run)
        .add_systems(Last, save_on_exit);
}

/// Write a change once it has settled.
fn save_when_settled(time: Res<Time<Real>>, mut store: ResMut<ProfileStore>) {
    if !store.dirty {
        return;
    }
    store.wait += time.delta_secs();
    if store.wait >= SAVE_DELAY {
        store.save_now();
    }
}

/// A change still waiting is written before the app closes.
fn save_on_exit(mut exit: MessageReader<AppExit>, mut store: ResMut<ProfileStore>) {
    if exit.read().count() > 0 && store.dirty {
        store.save_now();
    }
}

/// The run that just ended goes on the record.
fn record_run(
    run: Res<Run>,
    selection: Res<Selection>,
    session: Res<Session>,
    mut store: ResMut<ProfileStore>,
) {
    if !run.begun {
        return;
    }
    let result = RunResult {
        zone: selection.stage_def().name.to_string(),
        won: run.won,
        secs: run.elapsed.min(session.rules.duration),
        level: run.level,
        kills: run.kills,
        damage_done: run.damage_done,
    };
    store.edit(|p| p.record(&result));
    // A finished run is worth writing at once.
    store.save_now();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(zone: &str, won: bool, level: u32, kills: u32) -> RunResult {
        RunResult {
            zone: zone.into(),
            won,
            secs: 300.0,
            level,
            kills,
            damage_done: 1000.0,
        }
    }

    #[test]
    fn a_profile_round_trips() {
        let mut p = Profile::default();
        p.hero = HeroPick {
            class: "Shaman".into(),
            race: 8,
            sex: 1,
            zone: "Elwynn Forest".into(),
        };
        p.record(&result("Elwynn Forest", true, 12, 300));
        assert_eq!(Profile::parse(&p.to_text()).unwrap(), p);
    }

    #[test]
    fn missing_fields_default_and_unknown_ones_are_skipped() {
        let p = Profile::parse("version = 1\n[hero]\nclass = \"Mage\"\n").unwrap();
        assert_eq!(p.hero.class, "Mage");
        assert_eq!(p.totals, Totals::default());
        // A field a newer game added does not stop an older one reading the rest.
        let p = Profile::parse("version = 1\ngold = 500\n[hero]\nclass = \"Rogue\"\n").unwrap();
        assert_eq!(p.hero.class, "Rogue");
        assert!(!p.from_newer_game());
        // An empty file is a fresh profile.
        assert_eq!(Profile::parse("").unwrap(), Profile::default());
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(Profile::parse("this is = = not toml").is_err());
        assert!(Profile::parse("[hero]\nrace = \"eight\"").is_err());
    }

    #[test]
    fn a_newer_games_profile_is_flagged() {
        let p = Profile::parse(&format!("version = {}\n", PROFILE_VERSION + 1)).unwrap();
        assert!(p.from_newer_game());
    }

    #[test]
    fn runs_add_up_overall_and_per_zone() {
        let mut p = Profile::default();
        p.record(&result("Elwynn Forest", false, 8, 120));
        p.record(&result("Elwynn Forest", true, 15, 400));
        p.record(&result("Duskwood", false, 4, 30));
        assert_eq!(p.totals.runs, 3);
        assert_eq!(p.totals.wins, 1);
        assert_eq!(p.totals.kills, 550);
        assert_eq!(p.totals.best_level, 15);
        assert!((p.totals.time_played - 900.0).abs() < 1e-6);
        let e = &p.zones["Elwynn Forest"];
        assert_eq!(
            (e.runs, e.wins, e.best_level, e.best_kills),
            (2, 1, 15, 400)
        );
        assert_eq!(p.zones["Duskwood"].wins, 0);
    }

    #[test]
    fn the_old_pick_file_carries_over_by_name() {
        let p = Profile::from_old_pick("3 8 1 5").unwrap();
        assert_eq!(p.hero.class, data::CLASSES[3].name);
        assert_eq!(p.hero.zone, data::MAPS[5].name);
        assert_eq!((p.hero.race, p.hero.sex), (8, 1));
        assert_eq!(pick_indices(&p.hero), (Some(3), Some(5)));
        // Out of range or short: nothing to carry.
        assert!(Profile::from_old_pick("99 1 0 0").is_none());
        assert!(Profile::from_old_pick("1 2").is_none());
        assert!(Profile::from_old_pick("").is_none());
    }

    #[test]
    fn a_renamed_class_or_zone_falls_back_instead_of_landing_elsewhere() {
        let hero = HeroPick {
            class: "Death Knight".into(),
            zone: "Northrend".into(),
            ..HeroPick::default()
        };
        assert_eq!(pick_indices(&hero), (None, None));
    }

    #[test]
    fn edits_mark_the_store_dirty_only_when_something_changed() {
        let mut s = ProfileStore::ephemeral(Profile::default());
        s.edit(|_| {});
        assert!(!s.dirty);
        s.edit(|p| p.hero.class = "Druid".into());
        assert!(s.dirty);
        // An in-memory store never writes, but a save clears the flag.
        s.save_now();
        assert!(!s.dirty);
    }
}
