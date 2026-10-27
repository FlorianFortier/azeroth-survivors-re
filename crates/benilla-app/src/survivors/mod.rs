//! Warcraft Survivors: a top-down survival mode on the benilla world, started by
//! [`crate::run_survivors`]. There is no WoW server: the hero, the enemies and the spells are local
//! entities dressed by the same attach, animation and spell-visual systems a streamed unit gets,
//! on a battleground picked at the menu ([`data::MAPS`]), with the player controller and FrameXML
//! off ([`crate::run_mode::CaptureMode`]).
//!
//! Alone, this machine plays the run. In multiplayer ([`net`]) the lobby's host plays it for every
//! hero and streams it to the guests the way a WoW server streams a world; each guest steers its
//! own hero and sends it back.

mod abilities;
mod auto;
mod cards;
mod character;
mod data;
mod enemies;
mod frames;
mod grade;
mod hero;
mod hud;
mod kit;
mod lang;
mod legs;
mod market;
mod marks;
mod menu;
mod minimap;
mod mobility;
mod names;
mod net;
mod offer;
mod outfit;
mod profile;
mod rewards;
mod sheet;
mod stats;
mod sync;
mod talents;
mod targeting;
mod treasure;
mod units;

use benilla_world::schedule::WorldStage;
use bevy::prelude::*;

use data::{AbilityDef, Bonus, ClassDef, PassiveDef, Stat};
pub(super) use stats::Stats;

/// Set by [`crate::run_survivors`] before the app builds. wasm32 has no process environment, so
/// the flag lives here; `WOW_SURVIVORS` (`?survivors=1` / `__wenilla_env.survivors` on the web)
/// still turns it on from outside.
pub(crate) static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) fn active() -> bool {
    ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
        || crate::webenv::var("WOW_SURVIVORS").is_some()
}

pub(crate) struct SurvivorsPlugin;

impl Plugin for SurvivorsPlugin {
    fn build(&self, app: &mut App) {
        // Present before `OnEnter(InWorld)`, where the UI lifecycle decides not to load FrameXML.
        app.insert_resource(crate::run_mode::CaptureMode)
            .add_plugins(grade::plugin)
            .init_state::<Phase>()
            .init_resource::<Rng>()
            .init_resource::<Selection>()
            .init_resource::<Run>()
            .init_resource::<hero::CamRig>()
            .init_resource::<hero::MenuStage>()
            .init_resource::<hero::AimPoint>()
            .init_resource::<hero::MouseLook>()
            .init_resource::<targeting::HeroTarget>()
            .init_resource::<abilities::PendingHits>()
            .init_resource::<enemies::Spawner>()
            .init_resource::<treasure::Field>()
            .init_resource::<market::Market>()
            .init_resource::<legs::Legs>()
            .init_resource::<market::BuyQueue>()
            .init_resource::<hud::Offer>()
            .init_resource::<cards::PendingPick>()
            .init_resource::<Session>()
            .init_resource::<sync::LocalPicks>()
            .init_resource::<mobility::MobilityQueue>()
            .add_message::<abilities::Hit>()
            .add_message::<enemies::Loot>()
            .add_plugins(bevy::ui_render::UiMaterialPlugin::<hud::SweepUiMaterial>::default())
            .add_plugins(net::plugin)
            .add_plugins(profile::plugin)
            .add_plugins(kit::plugin)
            .add_plugins(character::plugin)
            .add_plugins(sheet::plugin)
            .add_plugins(rewards::plugin)
            .add_systems(Startup, hud::setup_hud)
            .add_systems(Update, (hero::daylight, treasure::find_looks))
            .add_systems(
                Update,
                (
                    hero::read_mouse,
                    hero::capture_cursor,
                    hero::turn_camera,
                    hero::aim_cursor,
                    mobility::cast_mobility,
                    hero::drive_hero,
                    hero::place_camera,
                    targeting::pick_target,
                    targeting::keep_target,
                )
                    .chain()
                    .in_set(WorldStage::Input),
            )
            .add_systems(
                Update,
                sync::begin_run
                    .after(WorldStage::Input)
                    .before(mobility::apply_mobility),
            )
            .add_systems(
                Update,
                (
                    mobility::apply_mobility,
                    enemies::spawn_enemies,
                    enemies::move_enemies,
                    abilities::cast_abilities,
                    abilities::tick_areas,
                    abilities::item_novas,
                    abilities::apply_hits,
                    enemies::tick_marks,
                    enemies::drop_loot,
                    enemies::collect_gems,
                    treasure::tend_field,
                    market::tend_market,
                    legs::tend_legs,
                    hero::hero_vitals,
                )
                    .chain()
                    .in_set(SimSet)
                    .after(WorldStage::Input)
                    // Solo in play; with others, through anyone's menu or cards. Entering a
                    // battleground raises its loading screen: the run waits behind it.
                    .run_if(sim_active),
            )
            .add_systems(
                Update,
                (enemies::reap_dead, enemies::still_gems).after(WorldStage::Input),
            )
            .add_systems(
                Update,
                (
                    hero::follow_map,
                    hud::menu_preview,
                    hud::buttons,
                    hud::keys,
                    hud::refresh_menu,
                    menu::restore_pick,
                    menu::build_menu,
                    menu::menu_clicks,
                    menu::refresh_menu_ui.before(crate::glue::GlueVisuals),
                    (
                        hud::follow_offer,
                        cards::deal_cards,
                        cards::choose_card,
                        cards::card_keys,
                        cards::card_buttons,
                        hud::reroll_hint,
                        cards::animate_cards,
                    )
                        .chain(),
                    hud::refresh_hud,
                    hud::refresh_results,
                    hud::fill_icons,
                    hud::update_cooldown_sweeps,
                    hud::refresh_mobility,
                    hud::show_panels,
                )
                    .chain()
                    .after(WorldStage::Input),
            )
            .add_systems(
                Update,
                (
                    hud::apply_ui_scale,
                    hud::fill_art,
                    hud::xp_hover,
                    hud::icon_tooltip,
                    hud::hero_plate,
                    frames::refresh_frames,
                    frames::refresh_next_boss,
                    marks::place_labels,
                    market::shop_panel,
                    marks::draw_rings,
                    minimap::map_buttons,
                    minimap::zone_text,
                    minimap::compose,
                    hero::drag_stage,
                    hero::frame_stage,
                )
                    .chain()
                    .after(hud::show_panels),
            )
            // The frame's last word: picks land, the view catches up, then the phase follows it.
            .add_systems(
                PostUpdate,
                (
                    sync::apply_local_picks,
                    offer::deal_offers,
                    sync::sync_view,
                    hero::level_up_gate,
                    hero::run_outcome,
                )
                    .chain(),
            )
            .add_systems(OnEnter(Phase::Playing), unpause)
            .add_systems(OnEnter(Phase::LevelUp), pause)
            .add_systems(OnEnter(Phase::Paused), pause)
            .add_systems(OnEnter(Phase::GameOver), unpause)
            .add_systems(OnEnter(Phase::Menu), (unpause, reset_run))
            .add_systems(OnEnter(Phase::Lobby), (unpause, reset_run));
        if let Some(auto) = auto::from_env() {
            app.insert_resource(auto)
                .add_systems(
                    Update,
                    auto::drive.after(hud::keys).before(hud::refresh_menu),
                )
                .add_systems(Update, auto::plan.before(WorldStage::Input));
        }
    }
}

/// The screen the mode is on.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(super) enum Phase {
    #[default]
    Menu,
    Playing,
    LevelUp,
    Paused,
    GameOver,
    /// A multiplayer lobby: its members, its rules, readying up.
    Lobby,
}

/// The systems that play the run's rules.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct SimSet;

/// Everything spawned for a run, despawned when the next begins.
#[derive(Component)]
pub(super) struct RunEntity;

/// The class, look and battleground picked at the menu; the battleground holds for the run.
#[derive(Resource, Default)]
pub(super) struct Selection {
    pub class: Option<usize>,
    /// `(display, race, sex)`, one per race and sex the class's trainers come in.
    pub looks: Vec<(u32, u8, u8)>,
    pub look: Option<usize>,
    /// The appearance dials picked for the look ([`outfit::Appearance`]).
    pub appearance: outfit::Appearance,
    /// The difficulty picked under Run Options ([`data::DIFFICULTIES`] index).
    pub difficulty: usize,
    /// What the profile brings the picked hero, for the lobby loadout.
    pub brings: Brings,
    /// An index into [`data::MAPS`].
    pub map: usize,
    /// The battleground the world is standing in: the last one entered (or the saved pick at
    /// boot). Picking another at the menu changes only `map`, so the menu hero stays put and no
    /// loading screen rises until Enter World.
    pub stage: usize,
    /// Bumped on every change, so the menu rebuilds once.
    pub version: u32,
}

impl Selection {
    pub fn class_def(&self) -> Option<&'static ClassDef> {
        self.class.map(|i| &data::CLASSES[i])
    }

    pub fn map_def(&self) -> &'static data::MapDef {
        &data::MAPS[self.map.min(data::MAPS.len() - 1)]
    }

    pub fn stage_def(&self) -> &'static data::MapDef {
        &data::MAPS[self.stage.min(data::MAPS.len() - 1)]
    }
}

/// What a hero brings from its profile beyond its race: bonuses as the wire carries them, the
/// gear worn by name, and the perks bought, `(key, rank)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Brings {
    pub bonuses: Vec<(u8, u8, i32)>,
    pub gear: Vec<String>,
    pub perks: Vec<(String, u8)>,
}

/// Who plays the run on this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(super) enum Mode {
    /// Alone: this machine simulates, nothing on the network.
    #[default]
    Solo,
    /// The lobby's host: this machine simulates and streams the run to the others.
    Host,
    /// A guest: the host's run is copied here; this machine moves its own hero only.
    Client,
}

/// A seat in a multiplayer run, as the lobby had it when the run started.
#[derive(Clone, Debug)]
pub(super) struct Seat {
    pub slot: u8,
    pub name: String,
    pub class: usize,
    pub display: u32,
    pub race: u8,
    pub sex: u8,
    pub appearance: outfit::Appearance,
    /// What its profile brings, as its loadout carried it.
    pub brings: Brings,
}

/// The run's mode, its rules, and the table it is played at.
#[derive(Resource, Default)]
pub(super) struct Session {
    pub mode: Mode,
    /// This machine's seat.
    pub slot: u8,
    pub rules: data::Rules,
    /// Every seat, the local one included; empty in solo.
    pub seats: Vec<Seat>,
    /// This machine sits in a multiplayer lobby: the menu's Enter goes back to it rather than
    /// starting a solo run.
    pub in_lobby: bool,
}

impl Session {
    pub fn multiplayer(&self) -> bool {
        self.mode != Mode::Solo
    }
    /// This machine plays the run's rules (solo or host), rather than copying them.
    pub fn simulates(&self) -> bool {
        self.mode != Mode::Client
    }
    pub fn name_of(&self, slot: u8) -> &str {
        self.seats
            .iter()
            .find(|s| s.slot == slot)
            .map_or("", |s| s.name.as_str())
    }
}

/// The local player steers their hero: in play, and in multiplayer also while choosing a card,
/// since the run does not stop for one player's level-up.
pub(super) fn controls_active(phase: Phase, session: &Session) -> bool {
    phase == Phase::Playing || (session.multiplayer() && phase == Phase::LevelUp)
}

/// The run's rules advance: solo only in play; with others, through anyone's level-up or menu.
fn sim_active(
    phase: Res<State<Phase>>,
    session: Res<Session>,
    run: Res<Run>,
    loading: Res<crate::loading_screen::LoadingScreen>,
) -> bool {
    let phase = *phase.get();
    let live = phase == Phase::Playing
        || (session.multiplayer() && matches!(phase, Phase::LevelUp | Phase::Paused));
    session.simulates() && live && run.begun && !run.waiting && !loading.covering()
}

/// An ability the hero has, with its own cooldown.
#[derive(Clone)]
pub(super) struct Owned {
    pub def: &'static AbilityDef,
    /// Its rank: the cards taken for it, the first that learned it. No cap.
    pub level: u16,
    pub cd: f32,
    /// What its cards gave it, summed ([`data::Perks`]).
    pub perks: data::Perks,
}

impl Owned {
    /// A newly learned ability, with what its card gave it.
    pub fn learned(def: &'static AbilityDef, perks: &[data::Perk], cd: f32) -> Self {
        let mut o = Self {
            def,
            level: 1,
            cd,
            perks: data::Perks::default(),
        };
        for &p in perks {
            o.perks = o.perks.with(p);
        }
        o
    }

    /// One rank more, with what its card gives.
    pub fn rank_up(&mut self, perks: &[data::Perk]) {
        self.level = self.level.saturating_add(1);
        for &p in perks {
            self.perks = self.perks.with(p);
        }
    }

    pub fn damage_mult(&self) -> f32 {
        1.0 + self.perks.damage
    }
    pub fn cooldown(&self, stats: &Stats) -> f32 {
        let base = self.def.cooldown * self.perks.cooldown;
        (base * (1.0 - stats.haste).max(0.35)).max(0.2)
    }
    pub fn count(&self) -> u32 {
        self.def.count + u32::from(self.perks.targets)
    }
    pub fn radius(&self, stats: &Stats) -> f32 {
        self.def.radius * (1.0 + self.perks.area) * stats.area
    }
    pub fn duration(&self) -> f32 {
        self.def.duration * (1.0 + self.perks.duration)
    }
}

pub(super) const BASE_HP: f32 = Stat::MaxHp.base();

/// One hero's run: the numbers the simulation plays on. Every hero in a run carries its own as a
/// [`HeroState`]; the local player's is mirrored into [`Run`] for the HUD.
#[derive(Clone)]
pub(super) struct Vitals {
    pub class: &'static ClassDef,
    pub race: u8,
    pub kills: u32,
    pub level: u32,
    pub xp: f32,
    pub hp: f32,
    pub abilities: Vec<Owned>,
    /// Each passive, its rank, and its ranks' worth (as [`Owned::power`]).
    pub passives: Vec<(&'static PassiveDef, u8, f32)>,
    pub shield: f32,
    pub shield_t: f32,
    pub immune_t: f32,
    pub thorns: f32,
    pub thorns_t: f32,
    pub thorns_spell: u32,
    pub heal_rate: f32,
    pub heal_t: f32,
    /// Seconds to the next nova of a run item's (`data::ItemEffect::Nova`); the simulation's.
    pub nova_t: f32,
    /// The class mobility's cooldown and running effect. Owned by the machine that moves the
    /// hero: the simulation only reads it.
    mobile: mobility::MobilityState,
    pub pending_levels: u32,
    /// Seconds since the hero fell, while the death plays out.
    pub dead_t: Option<f32>,
    /// Seconds an ally has stood over this fallen hero.
    pub revive_t: f32,
    pub damage_done: f64,
    /// The damage done this run, by the spell that dealt it, for the damage meter.
    pub damage_by: Vec<(u32, f64)>,
    /// Bumped whenever abilities or passives change, so the HUD rebuilds the icon rows once.
    pub version: u32,
    /// Moves at the level-up cards and blessings taken by this hero, so a client knows which of
    /// its own the host's numbers include.
    pub picks: u32,
    /// What the hero brings into the run from outside it: the collection, talents, racials and
    /// gear. Set when the run begins.
    pub meta: Vec<Bonus>,
    /// Effects that fire on their own, and the ones running now.
    pub procs: Vec<stats::Proc>,
    pub timed: Vec<stats::Timed>,
    /// The items this hero has found this run, kept on the profile when it ends.
    pub loot: Vec<&'static data::ItemDef>,
    /// The run's gold in hand, picked up from kills and spent on chests.
    pub gold: f32,
    /// The run's items and how many of each.
    pub run_items: Vec<(&'static data::RunItemDef, u8)>,
    /// The level-up cards dealt this hero, by the simulation: only these may be picked.
    pub offer: Vec<offer::Dealt>,
    /// What this hero banished: never dealt it again this run. The simulation's alone.
    pub banned: Vec<offer::Ban>,
    /// Rerolls and banishes in hand (a skip is always there to take).
    pub rerolls: u8,
    pub banishes: u8,
    /// More cards on the next level-up's deal: the Opening Hand perk's, spent on the first.
    pub opening_hand: u8,
    /// More gold a skipped hand pays: the Skip perk's.
    pub skip_gold: u32,
    /// Keys in hand: each opens a chest for nothing.
    pub keys: u8,
    /// The blessings a Shrine of Power offers this hero, by the simulation: one may be taken.
    pub shrine_offer: Vec<data::Boon>,
    /// What the field's shrines gave this hero for the rest of the run: blessings taken, and
    /// Greed's gold and the Curse's experience.
    pub boons: Vec<Bonus>,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            class: &data::CLASSES[0],
            race: 0,
            kills: 0,
            level: 1,
            xp: 0.0,
            hp: BASE_HP,
            abilities: Vec::new(),
            passives: Vec::new(),
            shield: 0.0,
            shield_t: 0.0,
            immune_t: 0.0,
            thorns: 0.0,
            thorns_t: 0.0,
            thorns_spell: 0,
            heal_rate: 0.0,
            heal_t: 0.0,
            nova_t: 0.0,
            mobile: mobility::MobilityState::default(),
            pending_levels: 0,
            dead_t: None,
            revive_t: 0.0,
            damage_done: 0.0,
            damage_by: Vec::new(),
            version: 0,
            picks: 0,
            meta: Vec::new(),
            procs: Vec::new(),
            timed: Vec::new(),
            loot: Vec::new(),
            gold: 0.0,
            run_items: Vec::new(),
            offer: Vec::new(),
            banned: Vec::new(),
            rerolls: data::REROLLS_PER_RUN,
            banishes: data::BANISHES_PER_RUN,
            opening_hand: 0,
            skip_gold: 0,
            keys: 0,
            shrine_offer: Vec::new(),
            boons: Vec::new(),
        }
    }
}

impl Vitals {
    /// A fresh hero of `class`: its starting ability and full health.
    pub fn new(class: &'static ClassDef, race: u8) -> Self {
        let mut v = Self {
            class,
            race,
            ..Self::default()
        };
        v.abilities
            .push(Owned::learned(data::ability(class.start), &[], 0.5));
        v.hp = v.stats().max_hp;
        v.version += 1;
        v
    }

    /// The hero's numbers now: the class's head start, the run's passives, what the hero
    /// brought in, and the effects running on it.
    pub fn stats(&self) -> Stats {
        let passives: Vec<Bonus> = self
            .passives
            .iter()
            .map(|&(p, _, power)| p.bonus_at(power))
            .collect();
        let items: Vec<Bonus> = self
            .run_items
            .iter()
            .map(|&(i, n)| stats::scaled(i.bonus, f32::from(n)))
            .collect();
        Stats::from_bonuses(
            data::class_bonuses(self.class.id)
                .iter()
                .chain(&passives)
                .chain(&items)
                .chain(&self.boons)
                .chain(&self.meta)
                .chain(self.timed.iter().flat_map(|t| t.bonuses)),
        )
    }

    /// What the hero's run items do beyond their stats, each at its stacks.
    pub fn item_effects(&self) -> impl Iterator<Item = data::ItemEffect> + '_ {
        self.run_items
            .iter()
            .filter_map(|&(i, n)| i.effect.map(|e| e.at(n)))
    }

    /// The hero with what it brings into the run, at full health on it.
    pub fn with_meta(mut self, meta: Vec<Bonus>, procs: &[&'static data::ProcDef]) -> Self {
        self.meta = meta;
        self.procs = procs.iter().map(|d| stats::Proc::new(d)).collect();
        self.hp = self.stats().max_hp;
        self
    }

    pub fn alive(&self) -> bool {
        self.dead_t.is_none()
    }
}

/// One hero in the run, on its unit: its seat at the table and its numbers.
#[derive(Component)]
pub(super) struct HeroState {
    pub slot: u8,
    pub v: Vitals,
}

impl std::ops::Deref for HeroState {
    type Target = Vitals;
    fn deref(&self) -> &Vitals {
        &self.v
    }
}

impl std::ops::DerefMut for HeroState {
    fn deref_mut(&mut self) -> &mut Vitals {
        &mut self.v
    }
}

/// The hero this machine moves: the camera follows it and WASD drives it.
#[derive(Component)]
pub(super) struct LocalHero;

/// Something the horde is made of, on the host's enemies and a client's copies of them alike:
/// what aim, the minimap and the mobility spells look for.
#[derive(Component)]
pub(super) struct Hostile;

/// A pickup whose model loops are held still.
#[derive(Component)]
pub(super) struct Pickup;

/// The run: its shared clock and announcements, and the local hero's numbers as [`Run::me`].
/// `Run` derefs to `me`, so the HUD reads the local hero as `run.hp`, `run.level` and so on.
#[derive(Resource)]
pub(super) struct Run {
    pub me: Vitals,
    pub elapsed: f32,
    pub bosses: u32,
    pub rings: u32,
    /// The stage's final boss has fallen: the stage is cleared (`legs.rs`).
    pub final_slain: bool,
    pub won: bool,
    /// Every hero has fallen and stayed down.
    pub lost: bool,
    pub next_guid: u64,
    /// A centre-screen announcement and its seconds left.
    pub banner: Option<(String, f32)>,
    /// The run's heroes exist and carry their numbers; false between [`hud::start_run`] and the
    /// first frame of play, when [`begin_run`] seats them.
    pub begun: bool,
    /// Multiplayer: still waiting for every player to load the zone. The clock holds.
    pub waiting: bool,
    /// Counts the runs started, so what belongs to one run knows when the next begins.
    pub run_id: u32,
    /// What the next chest costs (the host's, for a guest).
    pub chest_cost: u32,
    /// The field's elite posts (the host's, for a guest), for the marks.
    pub posts: Vec<treasure::PostView>,
    /// The field's shrines (the host's, for a guest), for the marks.
    pub shrines: Vec<treasure::ShrineView>,
    /// The merchant's wares (the host's, for a guest), for the shop panel.
    pub shop: Vec<market::Ware>,
    /// The Shrines of Greed and of Curses woken this run: each makes the foes tougher.
    pub greed: u8,
    pub curse: u8,
    /// The run's stage, from 0 ([`data::STAGES`] of them, each a zone): `elapsed` is its clock,
    /// and `past_secs` the stages before it.
    pub leg: u8,
    pub past_secs: f32,
    /// The stage's boss falls and its rewards are given: its portal onward (or the swarm) opens.
    pub leg_done: bool,
    /// The portals on the field (the host's, for a guest).
    pub gates: Vec<legs::GateView>,
    /// The final swarm's seconds left, while it runs.
    pub swarm: Option<f32>,
    /// The final swarm held out: the run is won at the end of the frame.
    pub cleared: bool,
    /// The [`data::DIFFICULTIES`] index the run is played at; Normal in a lobby, whose own rules
    /// set its difficulty.
    pub difficulty: usize,
}

impl Default for Run {
    fn default() -> Self {
        Self {
            me: Vitals::default(),
            elapsed: 0.0,
            bosses: 0,
            rings: 0,
            final_slain: false,
            won: false,
            lost: false,
            next_guid: 1,
            banner: None,
            begun: false,
            waiting: false,
            run_id: 0,
            chest_cost: data::chest_cost(0),
            posts: Vec::new(),
            shrines: Vec::new(),
            shop: Vec::new(),
            greed: 0,
            curse: 0,
            leg: 0,
            past_secs: 0.0,
            leg_done: false,
            gates: Vec::new(),
            swarm: None,
            cleared: false,
            difficulty: 0,
        }
    }
}

impl Run {
    /// The run's rules for the stage in play: its clock [`data::STAGE_LENGTH`] of the run's, the
    /// foes tougher with each stage past the first, with every Shrine of Greed and of Curses
    /// woken, and thicker while the final swarm runs.
    pub fn threat(&self, mut rules: data::Rules) -> data::Rules {
        let (g, c) = (i32::from(self.greed), i32::from(self.curse));
        let l = i32::from(self.leg);
        rules.duration *= data::STAGE_LENGTH;
        rules.health *=
            data::GREED_HEALTH.powi(g) * data::CURSE_HEALTH.powi(c) * data::STAGE_HEALTH.powi(l);
        rules.damage *= data::GREED_DAMAGE.powi(g) * data::STAGE_DAMAGE.powi(l);
        rules.density *= data::CURSE_DENSITY.powi(c) * data::STAGE_DENSITY.powi(l);
        if self.swarm.is_some() {
            rules.density *= data::SWARM_DENSITY;
        }
        rules
    }

    /// The run's whole time: the stages behind it and the one in play.
    pub fn total_secs(&self) -> f32 {
        self.past_secs + self.elapsed
    }
}

impl std::ops::Deref for Run {
    type Target = Vitals;
    fn deref(&self) -> &Vitals {
        &self.me
    }
}

impl std::ops::DerefMut for Run {
    fn deref_mut(&mut self) -> &mut Vitals {
        &mut self.me
    }
}

impl Run {
    pub fn guid(&mut self) -> u64 {
        self.next_guid += 1;
        self.next_guid
    }
}

/// xorshift64*: the mode's dice, seeded from the clock.
#[derive(Resource)]
pub(super) struct Rng(u64);

impl Default for Rng {
    fn default() -> Self {
        let seed = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(seed | 1)
    }
}

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in `[0, 1)`.
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }
    pub fn index(&mut self, len: usize) -> usize {
        (self.next_u64() % len.max(1) as u64) as usize
    }
}

/// A level-up or the menu stops the clock alone; with others the run goes on.
fn pause(session: Res<Session>, mut time: ResMut<Time<Virtual>>) {
    if !session.multiplayer() {
        time.pause();
    }
}

fn unpause(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}

/// Back at the menu: everything the last run spawned goes, and the run starts over.
fn reset_run(
    mut commands: Commands,
    spawned: Query<Entity, With<RunEntity>>,
    mut run: ResMut<Run>,
    mut pending: ResMut<abilities::PendingHits>,
    mut spawner: ResMut<enemies::Spawner>,
    mut selection: ResMut<Selection>,
    mut picks: ResMut<sync::LocalPicks>,
    mut mobility: ResMut<mobility::MobilityQueue>,
) {
    for e in &spawned {
        commands.entity(e).despawn();
    }
    // Guids stay unique across runs: caches keyed by guid may outlive a despawn by a frame.
    let next_guid = run.next_guid;
    *run = Run {
        next_guid,
        ..Run::default()
    };
    pending.0.clear();
    *spawner = enemies::Spawner::default();
    picks.0.clear();
    picks.1.clear();
    mobility.0.clear();
    // The preview hero is respawned for the look still picked.
    selection.version += 1;
}
