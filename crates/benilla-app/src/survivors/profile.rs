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

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::data;
use super::{Phase, Run, Selection};

/// Bumped when a field changes meaning; a new field with a default does not need it.
pub(super) const PROFILE_VERSION: u32 = 1;

const FILE: &str = "survivors-profile.toml";
/// The pick file the profile replaced, read once to carry the last hero over.
const OLD_PICK_FILE: &str = "survivors-pick.txt";
/// The zones of the pick file's day, by its index, to carry an old pick over by name.
const OLD_PICK_ZONES: [&str; 16] = [
    "The Barrens",
    "The Dark Portal",
    "Gates of Ahn'Qiraj",
    "Fire Plume Ridge",
    "Gurubashi Arena",
    "Elwynn Forest",
    "Durotar",
    "Teldrassil",
    "Dun Morogh",
    "Mulgore",
    "Tirisfal Glades",
    "Westfall",
    "Duskwood",
    "Redridge Mountains",
    "Mount Hyjal",
    "Naxxramas",
];
/// Zones the campaign renamed: a record kept under the old name moves to the new one.
const RENAMED_ZONES: [(&str, &str); 1] = [("Fire Plume Ridge", "Un'Goro Crater")];
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
    /// The hero's name, as typed or rolled on the main screen; empty for none yet.
    pub name: String,
    /// The difficulty picked under Run Options, by [`data::Difficulty::name`].
    pub difficulty: String,
    /// The character-creation dials: skin, face, hair style, hair colour, facial hair.
    pub appearance: [u8; 5],
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
    /// The hardest [`data::DIFFICULTIES`] index won here (meaningful once `wins` is above 0).
    pub best_difficulty: u8,
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
    /// Every item found, by [`data::ItemDef::name`], and how many times.
    pub inventory: BTreeMap<String, u32>,
    /// What the hero wears: item name by [`data::GearSlot::name`].
    pub equipped: BTreeMap<String, String>,
    /// Gold in hand, earned by runs, spent on talent points.
    pub gold: u64,
    /// Talent points trained, the account's; each class spends them on its own trees.
    pub talent_points: u32,
    /// Each class's talents, by [`data::ClassDef::name`]: talent id to rank.
    pub talents: BTreeMap<String, super::talents::Spent>,
    /// What the Codex has seen: the horde and bosses met, by name, and the spells and passives
    /// played, by key.
    pub codex: Codex,
    /// The achievements whose reward has been claimed, by key.
    pub claimed: BTreeSet<String>,
}

/// The Codex's pages, filled as the runs meet things.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub(super) struct Codex {
    pub horde: BTreeSet<String>,
    pub bosses: BTreeSet<String>,
    pub spells: BTreeSet<String>,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            version: PROFILE_VERSION,
            hero: HeroPick::default(),
            totals: Totals::default(),
            zones: BTreeMap::new(),
            inventory: BTreeMap::new(),
            equipped: BTreeMap::new(),
            gold: 0,
            talent_points: 0,
            talents: BTreeMap::new(),
            codex: Codex::default(),
            claimed: BTreeSet::new(),
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
    /// The items the hero found, by name.
    pub loot: Vec<String>,
    /// The gold the run earned.
    pub gold: u32,
    /// The [`data::DIFFICULTIES`] index it was played at.
    pub difficulty: u8,
    /// What it met, for the Codex: horde and bosses by name, spells and passives by key.
    pub met_horde: Vec<String>,
    pub met_bosses: Vec<String>,
    pub spells: Vec<String>,
}

impl Profile {
    /// Read a profile file's text: `Err` for text that is not a profile at all. Missing fields
    /// take their defaults and unknown ones are skipped, so any version's file loads.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut p: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        p.follow_renames();
        Ok(p)
    }

    /// Move what was kept under a zone's old name to its new one, adding to any record already
    /// there. A zone the campaign dropped keeps its record: nothing is lost, it is only not shown.
    fn follow_renames(&mut self) {
        for (old, new) in RENAMED_ZONES {
            if let Some(r) = self.zones.remove(old) {
                let into = self.zones.entry(new.to_string()).or_default();
                into.runs += r.runs;
                into.wins += r.wins;
                into.best_time = into.best_time.max(r.best_time);
                into.best_level = into.best_level.max(r.best_level);
                into.best_kills = into.best_kills.max(r.best_kills);
            }
            if self.hero.zone == old {
                self.hero.zone = new.to_string();
            }
        }
    }

    /// Whether a zone's final boss has ever fallen.
    pub fn won(&self, zone: &str) -> bool {
        self.zones.get(zone).is_some_and(|r| r.wins > 0)
    }

    /// Whether the campaign's zone `i` is open to this profile; `WOW_UNLOCK_ALL=1`
    /// (`?unlock_all=1`) opens them all, for testing.
    pub fn zone_open(&self, i: usize) -> bool {
        unlock_all() || data::unlocked(i, |z| self.won(z))
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
        if r.won {
            z.best_difficulty = z.best_difficulty.max(r.difficulty);
        }
        for name in &r.loot {
            *self.inventory.entry(name.clone()).or_default() += 1;
        }
        self.gold += u64::from(r.gold);
        self.codex.horde.extend(r.met_horde.iter().cloned());
        self.codex.bosses.extend(r.met_bosses.iter().cloned());
        self.codex.spells.extend(r.spells.iter().cloned());
    }

    /// Whether a difficulty is open in a zone: Normal always, any other once the one below it has
    /// been won there.
    pub fn difficulty_open(&self, zone: &str, difficulty: usize) -> bool {
        difficulty == 0
            || (difficulty < data::DIFFICULTIES.len()
                && self.zones.get(zone).is_some_and(|r| {
                    r.wins > 0 && usize::from(r.best_difficulty) + 1 >= difficulty
                }))
    }

    /// The hardest difficulty open in a zone.
    pub fn hardest_open(&self, zone: &str) -> usize {
        (0..data::DIFFICULTIES.len())
            .rev()
            .find(|&d| self.difficulty_open(zone, d))
            .unwrap_or(0)
    }

    /// How far an achievement has come: `(now, goal)`, both counted the same way.
    pub fn progress(&self, a: &data::AchievementDef) -> (u64, u64) {
        use data::Goal;
        let flag = |b: bool| (u64::from(b), 1);
        match a.goal {
            Goal::Runs(n) => (u64::from(self.totals.runs), u64::from(n)),
            Goal::Wins(n) => (u64::from(self.totals.wins), u64::from(n)),
            Goal::Kills(n) => (self.totals.kills, n),
            Goal::Level(n) => (u64::from(self.totals.best_level), u64::from(n)),
            Goal::Collect(n) => (self.collected().len() as u64, n as u64),
            Goal::BossesMet(n) => (self.codex.bosses.len() as u64, n as u64),
            Goal::Points(n) => (u64::from(self.talent_points), u64::from(n)),
            Goal::WinZone(z) => flag(data::MAPS.get(z).is_some_and(|m| self.won(m.name))),
            Goal::WinAt(d) => flag(
                self.zones
                    .values()
                    .any(|r| r.wins > 0 && usize::from(r.best_difficulty) >= d),
            ),
        }
    }

    /// Whether an achievement is done.
    pub fn achieved(&self, a: &data::AchievementDef) -> bool {
        let (now, goal) = self.progress(a);
        now >= goal
    }

    /// The achievements done and not yet claimed: the Rewards tab.
    pub fn unclaimed(&self) -> Vec<&'static data::AchievementDef> {
        data::ACHIEVEMENTS
            .iter()
            .filter(|a| self.achieved(a) && !self.claimed.contains(&a.key))
            .collect()
    }

    /// Claim an achievement's gold, once it is done; false if not done or already claimed.
    pub fn claim(&mut self, key: &str) -> bool {
        let Some(a) = data::ACHIEVEMENTS.iter().find(|a| a.key == key) else {
            return false;
        };
        if !self.achieved(a) || self.claimed.contains(key) {
            return false;
        }
        self.claimed.insert(key.to_string());
        self.gold += a.gold;
        true
    }

    /// Train the next talent point with gold, if there is enough and a point left.
    pub fn train_point(&mut self) -> bool {
        let cost = super::talents::point_cost(self.talent_points);
        if self.talent_points >= super::talents::MAX_POINTS || self.gold < cost {
            return false;
        }
        self.gold -= cost;
        self.talent_points += 1;
        true
    }

    /// The gear worn, slot by slot; a name the tables no longer have, or one no longer owned, is
    /// left off.
    pub fn gear(&self) -> Vec<&'static data::ItemDef> {
        data::GearSlot::ALL
            .iter()
            .filter_map(|s| {
                let name = self.equipped.get(s.name())?;
                let item = data::item(name)?;
                (item.slot == *s && self.inventory.get(name).is_some_and(|n| *n > 0))
                    .then_some(item)
            })
            .collect()
    }

    /// The items owned for a slot, in the campaign's order.
    pub fn owned_for(&self, slot: data::GearSlot) -> Vec<&'static data::ItemDef> {
        data::ITEMS
            .iter()
            .filter(|i| i.slot == slot && self.inventory.get(&i.name).is_some_and(|n| *n > 0))
            .collect()
    }

    /// The items found at least once, in the campaign's order: the collection.
    pub fn collected(&self) -> Vec<&'static data::ItemDef> {
        data::ITEMS
            .iter()
            .filter(|i| self.inventory.get(&i.name).is_some_and(|n| *n > 0))
            .collect()
    }

    /// What the collection gives every run.
    pub fn collection_bonuses(&self) -> Vec<data::Bonus> {
        self.collected()
            .into_iter()
            .map(data::collection_bonus)
            .collect()
    }

    /// Wear `item` in its slot, if it is owned.
    pub fn equip(&mut self, item: &data::ItemDef) {
        if self.inventory.get(&item.name).is_some_and(|n| *n > 0) {
            self.equipped
                .insert(item.slot.name().to_string(), item.name.clone());
        }
    }

    /// Take off what `slot` wears.
    pub fn unequip(&mut self, slot: data::GearSlot) {
        self.equipped.remove(slot.name());
    }

    /// What the profile brings into a run for a hero of `race` (the hero actually spawned, which
    /// a lobby may have changed): the bonuses that hold all run and the effects that fire on
    /// their own: the race's three racials, the gear worn, the collection, and the class's
    /// talents (with the client's talent tables).
    pub fn run_bonuses(
        &self,
        race: u8,
        class: &data::ClassDef,
        talents: Option<&benilla_formats::TalentCatalog>,
    ) -> (Vec<data::Bonus>, Vec<&'static data::ProcDef>) {
        let (mut bonuses, procs) = data::racial_bonuses(race);
        bonuses.extend(self.meta_bonuses(race, class, talents));
        (bonuses, procs)
    }

    /// What the profile itself brings, beyond the race's own: the gear worn, the collection and
    /// the class's talents. A guest's loadout carries these to the host.
    pub fn meta_bonuses(
        &self,
        race: u8,
        class: &data::ClassDef,
        talents: Option<&benilla_formats::TalentCatalog>,
    ) -> Vec<data::Bonus> {
        let mut bonuses = Vec::new();
        for item in self.gear() {
            bonuses.extend_from_slice(&item.bonuses);
        }
        bonuses.extend(self.collection_bonuses());
        if let (Some(catalog), Some(spent)) = (talents, self.talents.get(class.name)) {
            let trees = super::talents::trees(catalog, race, class.id);
            bonuses.extend(super::talents::bonuses(spent, &trees));
        }
        bonuses
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
        // By name, as the campaign reordered the zones; one it dropped starts at the first.
        let zone = OLD_PICK_ZONES
            .get(zone)
            .map(|old| {
                RENAMED_ZONES
                    .iter()
                    .find(|(o, _)| o == old)
                    .map_or(*old, |(_, n)| *n)
            })
            .and_then(|name| data::MAPS.iter().find(|m| m.name == name))
            .unwrap_or(&data::MAPS[0]);
        Some(Self {
            hero: HeroPick {
                class: class.name.to_string(),
                race: u8::try_from(race).ok()?,
                sex: u8::try_from(sex).ok()?,
                zone: zone.name.to_string(),
                name: String::new(),
                appearance: [0; 5],
                difficulty: String::new(),
            },
            ..Self::default()
        })
    }
}

/// `WOW_UNLOCK_ALL=1` (`?unlock_all=1` on the web): every zone open. The autoplayer
/// (`WOW_SURVIVORS_AUTO`) opens them too, as it plays whichever zone it is told.
pub(super) fn unlock_all() -> bool {
    static ALL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ALL.get_or_init(|| {
        crate::webenv::var("WOW_UNLOCK_ALL").is_some_and(|v| v.trim() == "1")
            || crate::webenv::var("WOW_SURVIVORS_AUTO").is_some()
    })
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
        .add_systems(
            Update,
            (save_when_settled, keep_to_open_zones, refresh_brings),
        )
        .add_systems(OnEnter(Phase::GameOver), record_run)
        .add_systems(Last, save_on_exit);
}

/// At the menu, the zone picked is one the campaign has opened: a pick restored from before a
/// lock, or made in the glue menu, moves to the furthest zone open. A lobby's zone is the host's
/// call and is left alone.
fn keep_to_open_zones(
    phase: Res<State<Phase>>,
    session: Res<super::Session>,
    store: Res<ProfileStore>,
    mut selection: ResMut<Selection>,
) {
    if *phase.get() != Phase::Menu || session.in_lobby || store.profile.zone_open(selection.map) {
        return;
    }
    let furthest = (0..data::MAPS.len())
        .rev()
        .find(|&i| store.profile.zone_open(i))
        .unwrap_or(0);
    selection.map = furthest;
    selection.version += 1;
}

/// What the picked hero brings from the profile, kept on the selection for the lobby loadout.
fn refresh_brings(
    store: Res<ProfileStore>,
    talents: Option<Res<crate::ui_talent::Talents>>,
    mut selection: ResMut<Selection>,
) {
    if !store.is_changed()
        && !selection.is_changed()
        && !talents.as_ref().is_some_and(|t| t.is_changed())
    {
        return;
    }
    let Some(class) = selection.class_def() else {
        return;
    };
    let race = super::menu::current_look(&selection).map_or(1, |l| l.0);
    let brings = super::Brings {
        bonuses: store
            .profile
            .meta_bonuses(race, class, talents.as_deref().map(|t| &t.catalog))
            .iter()
            .map(data::bonus_to_wire)
            .collect(),
        gear: store
            .profile
            .gear()
            .iter()
            .map(|i| i.name.clone())
            .collect(),
    };
    if selection.brings != brings {
        selection.brings = brings;
    }
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
    session: Res<super::Session>,
    mut store: ResMut<ProfileStore>,
) {
    if !run.begun {
        return;
    }
    let zone = selection.stage;
    let roster = data::roster(zone);
    let result = RunResult {
        zone: selection.stage_def().name.to_string(),
        won: run.won,
        secs: run.elapsed,
        level: run.level,
        kills: run.kills,
        damage_done: run.damage_done,
        loot: run.loot.iter().map(|i| i.name.clone()).collect(),
        gold: data::run_gold(
            run.kills,
            run.bosses,
            run.won,
            run.stats().gold
                * data::DIFFICULTIES[run.difficulty.min(data::DIFFICULTIES.len() - 1)].reward,
        ),
        difficulty: run.difficulty as u8,
        met_horde: data::met_horde(zone, run.elapsed, &session.rules)
            .into_iter()
            .map(String::from)
            .collect(),
        met_bosses: roster
            .bosses
            .iter()
            .take(run.bosses as usize)
            .map(|b| b.name.to_string())
            .collect(),
        spells: run
            .abilities
            .iter()
            .map(|o| o.def.key.to_string())
            .chain(run.passives.iter().map(|(p, _)| p.key.to_string()))
            .collect(),
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
            loot: Vec::new(),
            gold: 0,
            difficulty: 0,
            met_horde: Vec::new(),
            met_bosses: Vec::new(),
            spells: Vec::new(),
        }
    }

    fn warrior() -> &'static data::ClassDef {
        data::CLASSES.iter().find(|c| c.id == 1).unwrap()
    }

    #[test]
    fn the_codex_fills_as_runs_meet_things() {
        let mut p = Profile::default();
        let mut r = result("Elwynn Forest", false, 9, 99);
        r.met_horde = vec!["Young Wolf".into(), "Kobold Vermin".into()];
        r.met_bosses = vec!["Hogger".into()];
        r.spells = vec!["frostbolt".into()];
        p.record(&r);
        r.met_horde = vec!["Young Wolf".into(), "Prowler".into()];
        p.record(&r);
        assert_eq!(p.codex.horde.len(), 3);
        assert!(p.codex.bosses.contains("Hogger"));
        assert_eq!(Profile::parse(&p.to_text()).unwrap(), p);
    }

    #[test]
    fn achievements_are_earned_then_claimed_once() {
        let mut p = Profile::default();
        assert!(p.unclaimed().is_empty());
        assert!(!p.claim("first_steps"));
        p.record(&result("Elwynn Forest", true, 31, 1200));
        let keys: Vec<&str> = p.unclaimed().iter().map(|a| a.key.as_str()).collect();
        for k in [
            "first_steps",
            "victor",
            "slayer",
            "seasoned",
            "won Elwynn Forest",
        ] {
            assert!(keys.contains(&k), "{k} in {keys:?}");
        }
        assert!(!keys.contains(&"survivor"));
        let gold = p.gold;
        assert!(p.claim("first_steps"));
        assert_eq!(p.gold, gold + 50);
        assert!(!p.claim("first_steps"), "only once");
        assert!(!p.unclaimed().iter().any(|a| a.key == "first_steps"));
        let survivor = data::ACHIEVEMENTS
            .iter()
            .find(|a| a.key == "survivor")
            .unwrap();
        assert_eq!(p.progress(survivor), (1, 10));
        assert_eq!(Profile::parse(&p.to_text()).unwrap(), p);
    }

    #[test]
    fn difficulties_open_one_win_at_a_time() {
        let mut p = Profile::default();
        assert!(p.difficulty_open("Elwynn Forest", 0));
        assert!(!p.difficulty_open("Elwynn Forest", 1));
        p.record(&result("Elwynn Forest", false, 9, 99));
        assert_eq!(p.hardest_open("Elwynn Forest"), 0);
        p.record(&result("Elwynn Forest", true, 20, 900));
        assert_eq!(p.hardest_open("Elwynn Forest"), 1);
        let mut heroic = result("Elwynn Forest", true, 25, 1200);
        heroic.difficulty = 1;
        p.record(&heroic);
        assert_eq!(p.hardest_open("Elwynn Forest"), 2);
        // Another zone opens nothing here, and nothing past the table.
        assert_eq!(p.hardest_open("Duskwood"), 0);
        assert!(!p.difficulty_open("Elwynn Forest", 3));
    }

    #[test]
    fn gold_comes_from_runs_and_buys_points() {
        let mut p = Profile::default();
        assert!(!p.train_point());
        let mut r = result("Elwynn Forest", true, 20, 800);
        r.gold = 45;
        p.record(&r);
        assert_eq!(p.gold, 45);
        assert!(p.train_point());
        assert_eq!((p.gold, p.talent_points), (25, 1));
        // The second costs 30: not enough.
        assert!(!p.train_point());
        p.gold = 1_000_000;
        while p.train_point() {}
        assert_eq!(p.talent_points, super::super::talents::MAX_POINTS);
        assert_eq!(Profile::parse(&p.to_text()).unwrap(), p);
    }

    #[test]
    fn found_items_are_kept_and_worn_gear_counts() {
        let mut p = Profile::default();
        let helm = data::ITEMS
            .iter()
            .find(|i| i.slot == data::GearSlot::Head)
            .unwrap();
        let other = data::ITEMS
            .iter()
            .filter(|i| i.slot == data::GearSlot::Head)
            .nth(1)
            .unwrap();
        let mut r = result("Elwynn Forest", false, 5, 50);
        r.loot = vec![helm.name.clone(), helm.name.clone(), other.name.clone()];
        p.record(&r);
        assert_eq!(p.inventory[&helm.name], 2);
        assert!(p.gear().is_empty());
        // Found, so collected: two items, two collection bonuses, worn or not.
        assert_eq!(p.collected(), [helm, other]);
        assert_eq!(p.collection_bonuses().len(), 2);
        let bare = p.run_bonuses(1, warrior(), None).0.len();
        assert_eq!(bare, data::racial_bonuses(1).0.len() + 2);
        // Wearing one owned piece, then the other, then nothing.
        p.equip(helm);
        assert_eq!(p.gear(), [helm]);
        assert_eq!(
            p.run_bonuses(1, warrior(), None).0.len(),
            bare + helm.bonuses.len()
        );
        p.equip(other);
        assert_eq!(p.gear(), [other]);
        p.unequip(data::GearSlot::Head);
        assert!(p.gear().is_empty());
        // A piece not owned is not worn.
        let unowned = data::ITEMS
            .iter()
            .find(|i| i.slot == data::GearSlot::Feet)
            .unwrap();
        p.equip(unowned);
        assert!(p.equipped.get("Feet").is_none());
        // It survives the file.
        p.equip(helm);
        assert_eq!(Profile::parse(&p.to_text()).unwrap(), p);
        // Wearing by name, and taking off.
        p.equip(other);
        assert_eq!(p.gear(), [other]);
        p.unequip(data::GearSlot::Head);
        assert!(p.gear().is_empty());
        let boots = data::ITEMS
            .iter()
            .find(|i| i.slot == data::GearSlot::Feet)
            .unwrap();
        p.equip(boots);
        assert!(p.gear().is_empty(), "not owned, not worn");
        p.equip(helm);
        // A name worn but no longer in the tables is left off.
        p.equipped.insert("Legs".into(), "Thunderfury".into());
        assert_eq!(p.gear(), [helm]);
    }

    #[test]
    fn a_troll_brings_its_racials() {
        let p = Profile::default();
        let (bonuses, procs) = p.run_bonuses(8, warrior(), None);
        assert!(bonuses.contains(&data::Bonus::flat(data::Stat::Regen, 1.5)));
        assert!(bonuses.contains(&data::Bonus::flat(data::Stat::Crit, 0.03)));
        assert_eq!(
            procs.iter().map(|d| d.key).collect::<Vec<_>>(),
            ["berserking"]
        );
        // A Gnome's racials always hold, none fires.
        let (bonuses, procs) = p.run_bonuses(7, warrior(), None);
        assert_eq!(bonuses.len(), 3);
        assert!(procs.is_empty());
    }

    #[test]
    fn a_profile_round_trips() {
        let mut p = Profile::default();
        p.hero = HeroPick {
            class: "Shaman".into(),
            race: 8,
            sex: 1,
            zone: "Elwynn Forest".into(),
            name: "Nirkashed".into(),
            appearance: [3, 1, 4, 1, 5],
            difficulty: "Heroic".into(),
        };
        p.record(&result("Elwynn Forest", true, 12, 300));
        assert_eq!(Profile::parse(&p.to_text()).unwrap(), p);
    }

    #[test]
    fn missing_fields_default_and_unknown_ones_are_skipped() {
        let p = Profile::parse("version = 1\n[hero]\nclass = \"Mage\"\n").unwrap();
        assert_eq!(p.hero.class, "Mage");
        assert_eq!(p.hero.appearance, [0; 5]);
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
    fn a_renamed_zone_keeps_its_record() {
        let text = r#"
version = 1
[hero]
zone = "Fire Plume Ridge"
[zones."Fire Plume Ridge"]
runs = 3
wins = 1
best_level = 30
[zones."Un'Goro Crater"]
runs = 1
best_level = 12
[zones."The Barrens"]
runs = 2
"#;
        let p = Profile::parse(text).unwrap();
        assert_eq!(p.hero.zone, "Un'Goro Crater");
        assert!(!p.zones.contains_key("Fire Plume Ridge"));
        let u = &p.zones["Un'Goro Crater"];
        assert_eq!((u.runs, u.wins, u.best_level), (4, 1, 30));
        // A zone the campaign dropped keeps its record.
        assert_eq!(p.zones["The Barrens"].runs, 2);
    }

    #[test]
    fn winning_a_zone_opens_the_next() {
        let mut p = Profile::default();
        assert!(p.zone_open(0) && !p.zone_open(1));
        // A loss opens nothing.
        p.record(&result("Elwynn Forest", false, 10, 100));
        assert!(!p.zone_open(1));
        p.record(&result("Elwynn Forest", true, 20, 900));
        assert!(p.won("Elwynn Forest"));
        assert!(p.zone_open(1) && !p.zone_open(2));
    }

    #[test]
    fn the_old_pick_file_carries_over_by_name() {
        let p = Profile::from_old_pick("3 8 1 5").unwrap();
        assert_eq!(p.hero.class, data::CLASSES[3].name);
        // Index 5 was Elwynn Forest in the pick file's day.
        assert_eq!(p.hero.zone, "Elwynn Forest");
        assert_eq!((p.hero.race, p.hero.sex), (8, 1));
        assert_eq!(pick_indices(&p.hero), (Some(3), Some(0)));
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
