//! The run's field, Megabonk-style: chests to open with the gold the run picks up, and elites
//! standing guard at fixed posts around the battleground, each over a reward of its own (a roll
//! for gear, a spell's rank, a run item, a purse), coming back a while after they fall.
//!
//! Only solo and the host tend the field (it runs in `SimSet`); chests are game objects and
//! elites units, so guests see them as they see everything else. A chest opens for a hero who
//! stands at it with the gold for it, which needs no input of its own over the wire.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::data::{self, Reward, ShrineKind};
use super::enemies::{self, Enemy, GemKind};
use super::hero::Hero;
use super::units::{self, flat};
use super::{HeroState, Owned, Rng, Run, Selection, Session};

/// The game-object displays the client's tables have for a chest and a coin, found by model name.
#[derive(Clone, Copy, Default)]
struct Looks {
    chest: Option<u32>,
    coin: Option<u32>,
    shrine: Option<u32>,
    jar: Option<u32>,
    crate_: Option<u32>,
}

static LOOKS: std::sync::OnceLock<Looks> = std::sync::OnceLock::new();

/// Look for a chest's and a coin's model once the archives are open.
pub(super) fn find_looks(assets: Option<Res<benilla_assets::WorldAssets>>) {
    use benilla_assets::LockRecover;
    if LOOKS.get().is_some() {
        return;
    }
    let Some(assets) = assets else { return };
    let catalog = {
        let mut chain = assets.chain.lock_recover();
        benilla_formats::load_gameobject_catalog(&mut chain)
    };
    let looks = match catalog {
        Ok(c) => {
            let mut all: Vec<(u32, String)> = c
                .iter()
                .map(|(id, p)| (id, p.to_ascii_lowercase()))
                .collect();
            all.sort_unstable_by_key(|(id, _)| *id);
            let find = |words: &[&str]| {
                words.iter().find_map(|w| {
                    all.iter()
                        .find(|(_, p)| p.rsplit(['\\', '/']).next().is_some_and(|f| f.contains(w)))
                        .map(|(id, _)| *id)
                })
            };
            Looks {
                chest: find(&["treasurechest", "chest"]),
                coin: find(&["goldcoin", "coin"]),
                shrine: find(&["shrine", "altar", "obelisk"]),
                jar: find(&["jar", "urn", "vase"]),
                crate_: find(&["crate", "barrel"]),
            }
        }
        Err(e) => {
            warn!("survivors: no game-object table for chests and coins: {e:#}");
            Looks::default()
        }
    };
    let _ = LOOKS.set(looks);
}

/// A coin's display and scale; a small wisp when the tables have no coin.
pub(super) fn coin_look() -> (u32, f32) {
    LOOKS
        .get()
        .and_then(|l| l.coin)
        .map_or((426, 0.7), |d| (d, 1.0))
}

/// A chest's display and scale; a big purple wisp when the tables have no chest.
fn chest_look() -> (u32, f32) {
    LOOKS
        .get()
        .and_then(|l| l.chest)
        .map_or((1307, 3.0), |d| (d, 1.2))
}

/// What a unit is to the field, on every machine: the host sets it, a guest reads it off the
/// spawn's role, and the marks (`marks.rs`) draw from it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FieldMark {
    Chest,
    /// A Shrine of Challenge's prize, opened for nothing.
    FreeChest,
    /// An elite of the [`data::ELITES`] kind.
    Elite(u8),
    /// The shady merchant (`market.rs`).
    Merchant,
}

/// One elite post as the marks see it: where, which kind, and how long until it is back (0 while
/// its elite stands). The host's [`Field`] on solo and the host, the host's stream on a guest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PostView {
    pub kind: usize,
    pub pos: Vec3,
    pub wait: f32,
}

/// A jar's display and scale; a small wisp when the tables have none.
pub(super) fn jar_look() -> (u32, f32) {
    LOOKS
        .get()
        .and_then(|l| l.jar)
        .map_or((426, 2.0), |d| (d, 1.0))
}

/// A crate's display and scale; a small green wisp when the tables have none.
pub(super) fn crate_look() -> (u32, f32) {
    LOOKS
        .get()
        .and_then(|l| l.crate_)
        .map_or((1267, 2.0), |d| (d, 1.0))
}

/// A shrine's display and scale; a tall golden wisp when the tables have no shrine.
fn shrine_look() -> (u32, f32) {
    LOOKS
        .get()
        .and_then(|l| l.shrine)
        .map_or((1307, 4.0), |d| (d, 1.0))
}

/// Where a shrine stands in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShrineState {
    /// Waiting for a hero to stand in it.
    Ready,
    /// A Shrine of Challenge whose elites are still up.
    Fighting,
    /// Done with: it gave what it gives.
    Spent,
}

impl ShrineState {
    pub fn to_wire(self) -> u8 {
        self as u8
    }
    pub fn from_wire(n: u8) -> ShrineState {
        match n {
            0 => ShrineState::Ready,
            1 => ShrineState::Fighting,
            _ => ShrineState::Spent,
        }
    }
}

/// One shrine as the marks see it: which, where, how charged (0..1) and where in its life. The
/// host's [`Field`] on solo and the host, the host's stream on a guest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShrineView {
    pub kind: usize,
    pub pos: Vec3,
    pub charge: f32,
    pub state: ShrineState,
}

/// One shrine of the field, as the host tends it.
struct Shrine {
    view: ShrineView,
    /// Its model, gone once it is spent.
    model: Option<Entity>,
    /// A Shrine of Challenge's elites, while they are up.
    foes: Vec<Entity>,
}

/// A chest on the field, and how long a hero has stood at it.
#[derive(Component)]
pub(super) struct Chest {
    /// A Shrine of Challenge's prize: opens for nothing, and leaves the price where it was.
    free: bool,
    hold: f32,
    /// The hero told it lacks the gold, so the banner says it once a visit.
    told: bool,
}

/// One elite's post.
struct Post {
    kind: usize,
    pos: Vec3,
    unit: Option<Entity>,
    /// Seconds until it comes back, while it is down.
    wait: f32,
}

/// The field of the run in play.
#[derive(Resource, Default)]
pub(super) struct Field {
    /// The run it was laid out for ([`Run::run_id`]); another run lays it out anew.
    run_id: Option<u32>,
    opened: u32,
    posts: Vec<Post>,
    /// Seconds until each opened chest comes back elsewhere.
    returning: Vec<f32>,
    shrines: Vec<Shrine>,
}

/// A spot of the battleground, `t` of the way out to its leash, at angle `a`, on the ground.
pub(super) fn spot(world: &WorldPoint, center: Vec3, leash: f32, a: f32, t: f32) -> Vec3 {
    let mut p = center + Vec3::new(a.cos(), 0.0, a.sin()) * (leash * t);
    if let Some(h) = world.terrain_height_under(p) {
        p.y = h;
    }
    p
}

fn spawn_chest(commands: &mut Commands, run: &mut Run, pos: Vec3, free: bool) {
    let (display, scale) = chest_look();
    let e = units::spawn_object(commands, run.guid(), display, scale, pos);
    commands.entity(e).insert((
        Chest {
            free,
            hold: 0.0,
            told: false,
        },
        super::Pickup,
        if free {
            FieldMark::FreeChest
        } else {
            FieldMark::Chest
        },
    ));
}

/// Give a hero a reward that applies at once (a run item, a spell's rank, gold); returns the
/// banner's line. A roll for gear is a drop, not a grant: the caller spawns it.
pub(super) fn grant(hero: &mut HeroState, reward: Reward, rng: &mut Rng) -> String {
    match reward {
        Reward::RunItem => {
            // Five kinds at most: once they are held, the draw is among them, and gold once each
            // is at its most stacks.
            let item = if hero.run_items.len() >= data::MAX_RUN_ITEM_KINDS {
                let open: Vec<&'static data::RunItemDef> = hero
                    .run_items
                    .iter()
                    .filter(|(_, n)| *n < data::MAX_RUN_ITEM_STACKS)
                    .map(|(i, _)| *i)
                    .collect();
                match data::draw_held_run_item(&open, rng.f32()) {
                    Some(item) => item,
                    None => return grant(hero, Reward::Gold(data::FULL_RUN_ITEMS_GOLD), rng),
                }
            } else {
                data::draw_run_item(rng.f32(), rng.f32())
            };
            match hero.run_items.iter_mut().find(|(i, _)| *i == item) {
                Some((_, n)) => *n = (*n + 1).min(data::MAX_RUN_ITEM_STACKS),
                None => hero.run_items.push((item, 1)),
            }
            hero.version += 1;
            let n = hero
                .run_items
                .iter()
                .find(|(i, _)| *i == item)
                .map_or(1, |(_, n)| *n);
            format!("{} ×{n}", item.name)
        }
        Reward::SpellRank => {
            let open: Vec<usize> = hero
                .abilities
                .iter()
                .enumerate()
                .filter(|(_, o)| o.level < data::MAX_ABILITY_LEVEL)
                .map(|(k, _)| k)
                .collect();
            if open.is_empty() {
                return grant(hero, Reward::RunItem, rng);
            }
            let k = open[rng.index(open.len())];
            let o: &mut Owned = &mut hero.abilities[k];
            o.level += 1;
            let text = format!("{} rank {}", o.def.key.replace('_', " "), o.level);
            hero.version += 1;
            text
        }
        Reward::Gold(n) => {
            let mult = hero.stats().gold;
            hero.gold += n as f32 * mult;
            format!("{} gold", (n as f32 * mult).round())
        }
        Reward::Gear { .. } => String::new(),
    }
}

/// Lay out the field at a run's start, bring elites and chests back on their timers, and open a
/// chest for a hero who stands at it with the gold.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn tend_field(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    session: Res<Session>,
    selection: Res<Selection>,
    mut rng: ResMut<Rng>,
    mut field: ResMut<Field>,
    world: WorldPoint,
    mut chests: Query<(Entity, &Transform, &mut Chest)>,
    mut heroes: Query<(&Transform, &mut HeroState), (With<Hero>, Without<Chest>)>,
    foes: Query<&Enemy>,
) {
    if !run.begun {
        return;
    }
    let dt = time.delta_secs();
    let map = selection.stage_def();
    let center = super::hero::arena(map);
    let zone = selection.stage;
    let rules = run.threat(session.rules);

    // A new run: the elites take their posts and the chests their places.
    if field.run_id != Some(run.run_id) {
        *field = Field {
            run_id: Some(run.run_id),
            ..Field::default()
        };
        let turn = rng.range(0.0, std::f32::consts::TAU);
        for (k, _) in data::ELITES.iter().enumerate() {
            let a = turn + k as f32 / data::ELITES.len() as f32 * std::f32::consts::TAU;
            field.posts.push(Post {
                kind: k,
                pos: spot(&world, center, map.leash, a, 0.7),
                unit: None,
                wait: 0.0,
            });
        }
        for _ in 0..data::CHESTS {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let t = rng.range(0.25, 0.9);
            let pos = spot(&world, center, map.leash, a, t);
            spawn_chest(&mut commands, &mut run, pos, false);
        }
        // The shrines, each kind as many times as its table says, between the middle and the posts.
        for (k, def) in data::SHRINES.iter().enumerate() {
            for _ in 0..def.count {
                let a = rng.range(0.0, std::f32::consts::TAU);
                let t = rng.range(0.35, 0.85);
                let pos = spot(&world, center, map.leash, a, t);
                let (display, scale) = shrine_look();
                let model = units::spawn_object(&mut commands, run.guid(), display, scale, pos);
                field.shrines.push(Shrine {
                    view: ShrineView {
                        kind: k,
                        pos,
                        charge: 0.0,
                        state: ShrineState::Ready,
                    },
                    model: Some(model),
                    foes: Vec::new(),
                });
            }
        }
        run.banner = Some((
            "Elites guard the field, and chests wait for your gold.".into(),
            5.0,
        ));
    }

    // The elites: a fallen one comes back on its kind's timer.
    for i in 0..field.posts.len() {
        let down = field.posts[i]
            .unit
            .is_some_and(|e| foes.get(e).map_or(true, |en| en.dead));
        if down {
            let kind = field.posts[i].kind;
            field.posts[i].unit = None;
            field.posts[i].wait = data::ELITES[kind].respawn;
        }
        if field.posts[i].unit.is_some() {
            continue;
        }
        field.posts[i].wait -= dt;
        if field.posts[i].wait > 0.0 {
            continue;
        }
        let (kind, pos) = (field.posts[i].kind, field.posts[i].pos);
        let def = data::elite(zone, kind);
        let e = enemies::spawn_enemy(
            &mut commands,
            &mut run,
            &rules,
            def,
            pos,
            center,
            None,
            false,
        );
        commands
            .entity(e)
            .insert(FieldMark::Elite(kind as u8))
            .entry::<Enemy>()
            .and_modify(move |mut en| {
                en.elite = Some(kind as u8);
                en.home = Some(pos);
            });
        field.posts[i].unit = Some(e);
    }

    // Opened chests come back elsewhere.
    let mut back = 0;
    field.returning.retain_mut(|t| {
        *t -= dt;
        if *t <= 0.0 {
            back += 1;
            false
        } else {
            true
        }
    });
    for _ in 0..back {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let t = rng.range(0.25, 0.9);
        let pos = spot(&world, center, map.leash, a, t);
        spawn_chest(&mut commands, &mut run, pos, false);
    }

    // A hero standing at a chest with the gold opens it.
    let cost = data::chest_cost(field.opened);
    for (e, tf, mut chest) in &mut chests {
        let at = tf.translation;
        let near = heroes
            .iter_mut()
            .filter(|(t, h)| h.alive() && flat(t.translation - at).length() <= data::CHEST_REACH)
            .min_by(|a, b| {
                flat(a.0.translation - at)
                    .length()
                    .total_cmp(&flat(b.0.translation - at).length())
            });
        let Some((_, mut hero)) = near else {
            chest.hold = 0.0;
            chest.told = false;
            continue;
        };
        // A key opens it for nothing, and its price stays where it was.
        let keyed = !chest.free && hero.keys > 0;
        let cost = if chest.free || keyed { 0 } else { cost };
        if hero.gold + 1e-3 < cost as f32 {
            chest.hold = 0.0;
            if !chest.told {
                chest.told = true;
                run.banner = Some((
                    format!(
                        "This chest costs {cost} gold: you have {}.",
                        hero.gold.floor()
                    ),
                    2.5,
                ));
            }
            continue;
        }
        chest.hold += dt;
        if chest.hold < data::CHEST_OPEN_SECS {
            continue;
        }
        hero.gold -= cost as f32;
        if keyed {
            hero.keys -= 1;
            hero.version += 1;
        }
        if !chest.free {
            if !keyed {
                field.opened += 1;
            }
            field.returning.push(data::CHEST_RESPAWN_SECS);
        }
        commands.entity(e).despawn();
        let text = match data::chest_reward(rng.f32()) {
            Reward::Gear { percent } => {
                let gear = (rng.index(100) < usize::from(percent))
                    .then(|| data::horde_item(zone, rng.f32() / data::HORDE_ITEM_ODDS as f32))
                    .flatten();
                match gear {
                    Some(item) => {
                        enemies::spawn_gem(&mut commands, run.guid(), at, GemKind::Item(item));
                        format!("{} dropped!", item.name)
                    }
                    None => grant(&mut hero, Reward::Gold(cost.max(20)), &mut rng),
                }
            }
            reward => grant(&mut hero, reward, &mut rng),
        };
        run.banner = Some((format!("Chest: {text}"), 4.0));
        // One chest a frame: the next one's price is the new one.
        break;
    }
    run.chest_cost = data::chest_cost(field.opened);
    tend_shrines(
        &mut commands,
        &mut run,
        &rules,
        &mut rng,
        &mut field,
        zone,
        center,
        dt,
        &mut heroes,
        &foes,
    );
    run.shrines.clear();
    run.shrines.extend(field.shrines.iter().map(|s| s.view));
    run.posts.clear();
    run.posts.extend(field.posts.iter().map(|p| PostView {
        kind: p.kind,
        pos: p.pos,
        wait: if p.unit.is_some() {
            0.0
        } else {
            p.wait.max(0.0)
        },
    }));
}

/// Three blessings of different stats, each at a drawn rarity.
pub(super) fn deal_boons(rng: &mut Rng) -> Vec<data::Boon> {
    let mut stats: Vec<usize> = (0..data::BOONS.len()).collect();
    let mut out = Vec::with_capacity(3);
    while out.len() < 3 && !stats.is_empty() {
        let k = stats.swap_remove(rng.index(stats.len()));
        out.push(data::boon(k, data::CardRarity::draw(rng.f32())));
    }
    out
}

/// Take the blessing of the hero's offer that `wire` names (as `data::bonus_to_wire` writes it),
/// if it is one: the offer goes, the blessing stays for the run.
pub(super) fn take_boon(v: &mut super::Vitals, wire: (u8, u8, i32)) -> bool {
    let Some(b) = v
        .shrine_offer
        .iter()
        .find(|b| data::bonus_to_wire(&b.bonus) == wire)
        .copied()
    else {
        return false;
    };
    v.boons.push(b.bonus);
    v.shrine_offer.clear();
    v.version += 1;
    // A move like a card's: a guest's view knows which of its moves the host has applied.
    v.picks += 1;
    true
}

/// The shrines: a hero standing in one charges it (it ebbs when left alone), and a charged one
/// wakes: a Shrine of Power offers that hero three blessings, a Shrine of Challenge calls its
/// elites and leaves a free chest once they fall, Greed and Curses touch every hero and the foes.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tend_shrines(
    commands: &mut Commands,
    run: &mut Run,
    rules: &data::Rules,
    rng: &mut Rng,
    field: &mut Field,
    zone: usize,
    center: Vec3,
    dt: f32,
    heroes: &mut Query<(&Transform, &mut HeroState), (With<Hero>, Without<Chest>)>,
    foes: &Query<&Enemy>,
) {
    for sh in &mut field.shrines {
        let def = data::shrine(sh.view.kind);
        match sh.view.state {
            ShrineState::Spent => {}
            ShrineState::Fighting => {
                let up = sh
                    .foes
                    .iter()
                    .any(|&e| foes.get(e).is_ok_and(|en| !en.dead));
                if !up {
                    sh.foes.clear();
                    sh.view.state = ShrineState::Spent;
                    if let Some(m) = sh.model.take() {
                        commands.entity(m).despawn();
                    }
                    spawn_chest(commands, run, sh.view.pos, true);
                    run.banner = Some(("The challenge is won: a chest for nothing!".into(), 4.0));
                }
            }
            ShrineState::Ready => {
                let at = sh.view.pos;
                let inside = heroes
                    .iter_mut()
                    .filter(|(t, h)| {
                        h.alive() && flat(t.translation - at).length() <= data::SHRINE_RADIUS
                    })
                    .min_by(|a, b| {
                        flat(a.0.translation - at)
                            .length()
                            .total_cmp(&flat(b.0.translation - at).length())
                    });
                let Some((_, mut hero)) = inside else {
                    sh.view.charge =
                        (sh.view.charge - dt * data::SHRINE_DECAY / def.hold.max(0.1)).max(0.0);
                    continue;
                };
                // One offer at a time: a hero still choosing does not charge another.
                if def.kind == ShrineKind::Charge && !hero.shrine_offer.is_empty() {
                    continue;
                }
                sh.view.charge = (sh.view.charge + dt / def.hold.max(0.1)).min(1.0);
                if sh.view.charge < 1.0 {
                    continue;
                }
                match def.kind {
                    ShrineKind::Charge => {
                        hero.v.shrine_offer = deal_boons(rng);
                        hero.version += 1;
                        sh.view.state = ShrineState::Spent;
                        run.banner =
                            Some(("The shrine is charged: choose a blessing.".into(), 3.0));
                    }
                    ShrineKind::Challenge => {
                        for k in 0..data::CHALLENGE_ELITES {
                            let kind = rng.index(data::ELITES.len());
                            let a =
                                k as f32 / data::CHALLENGE_ELITES as f32 * std::f32::consts::TAU;
                            let pos = at + Vec3::new(a.cos(), 0.0, a.sin()) * 7.0;
                            let e = enemies::spawn_enemy(
                                commands,
                                run,
                                rules,
                                data::elite(zone, kind),
                                pos,
                                center,
                                None,
                                false,
                            );
                            commands
                                .entity(e)
                                .insert(FieldMark::Elite(kind as u8))
                                .entry::<Enemy>()
                                .and_modify(move |mut en| {
                                    en.elite = Some(kind as u8);
                                    en.home = Some(at);
                                });
                            sh.foes.push(e);
                        }
                        sh.view.state = ShrineState::Fighting;
                        run.banner =
                            Some(("The Shrine of Challenge calls its elites!".into(), 4.0));
                    }
                    ShrineKind::Greed | ShrineKind::Curse => {
                        let boon = if def.kind == ShrineKind::Greed {
                            run.greed = run.greed.saturating_add(1);
                            data::GREED_GOLD
                        } else {
                            run.curse = run.curse.saturating_add(1);
                            data::CURSE_GROWTH
                        };
                        for (_, mut h) in heroes.iter_mut() {
                            h.v.boons.push(boon);
                            h.version += 1;
                        }
                        sh.view.state = ShrineState::Spent;
                        run.banner = Some((
                            if def.kind == ShrineKind::Greed {
                                "Greed: more gold for everyone, and fiercer foes."
                            } else {
                                "A curse: more experience for everyone, and more foes."
                            }
                            .into(),
                            4.0,
                        ));
                    }
                }
                if sh.view.state == ShrineState::Spent {
                    if let Some(m) = sh.model.take() {
                        commands.entity(m).despawn();
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Vitals;
    use bevy::ecs::system::RunSystemOnce;

    fn hero() -> HeroState {
        let mage = data::CLASSES.iter().find(|c| c.name == "Mage").unwrap();
        HeroState {
            slot: 0,
            v: Vitals::new(mage, 1),
        }
    }

    #[test]
    fn run_items_stack_and_count_in_the_stats() {
        let mut h = hero();
        let mut rng = Rng::seeded(7);
        let before = h.stats();
        for _ in 0..40 {
            grant(&mut h, Reward::RunItem, &mut rng);
        }
        let stacks: u32 = h.run_items.iter().map(|(_, n)| u32::from(*n)).sum();
        assert!(stacks >= 30, "{stacks}");
        assert!(
            h.run_items.len() <= data::MAX_RUN_ITEM_KINDS,
            "five kinds at most"
        );
        assert!(h
            .run_items
            .iter()
            .all(|(_, n)| *n <= data::MAX_RUN_ITEM_STACKS));
        assert_ne!(h.stats(), before, "the items move the numbers");
    }

    #[test]
    fn five_kinds_held_take_more_of_themselves_then_gold() {
        let mut h = hero();
        let mut rng = Rng::seeded(11);
        let kinds: Vec<_> = data::RUN_ITEMS
            .iter()
            .take(data::MAX_RUN_ITEM_KINDS)
            .collect();
        for i in &kinds {
            h.run_items.push((i, 1));
        }
        for _ in 0..20 {
            grant(&mut h, Reward::RunItem, &mut rng);
        }
        assert_eq!(h.run_items.len(), data::MAX_RUN_ITEM_KINDS, "no sixth kind");
        assert!(
            h.run_items.iter().any(|(_, n)| *n > 1),
            "the held ones grow"
        );
        // Every kind at its most: gold instead.
        for (_, n) in &mut h.run_items {
            *n = data::MAX_RUN_ITEM_STACKS;
        }
        let gold = h.gold;
        grant(&mut h, Reward::RunItem, &mut rng);
        assert!(h.gold > gold);
    }

    #[test]
    fn a_spell_rank_lands_on_a_spell_until_none_has_room() {
        let mut h = hero();
        let mut rng = Rng::seeded(3);
        let level = h.abilities[0].level;
        grant(&mut h, Reward::SpellRank, &mut rng);
        assert_eq!(h.abilities[0].level, level + 1);
        h.abilities[0].level = data::MAX_ABILITY_LEVEL;
        // Every spell maxed: a run item instead.
        let text = grant(&mut h, Reward::SpellRank, &mut rng);
        assert!(!h.run_items.is_empty(), "{text}");
    }

    #[test]
    fn gold_rewards_follow_the_gold_stat() {
        let mut h = hero();
        let mut rng = Rng::seeded(1);
        grant(&mut h, Reward::Gold(40), &mut rng);
        assert!((h.gold - 40.0).abs() < 1e-3);
        h.run_items.push((data::run_item("lucky_coin").unwrap(), 2));
        grant(&mut h, Reward::Gold(10), &mut rng);
        assert!(h.gold > 50.0);
    }

    #[test]
    fn blessings_are_three_stats_and_only_one_of_them_is_taken() {
        let mut rng = Rng::seeded(11);
        let mut h = hero();
        let offer = deal_boons(&mut rng);
        assert_eq!(offer.len(), 3);
        for (i, a) in offer.iter().enumerate() {
            assert!(offer[i + 1..].iter().all(|b| b.bonus.stat != a.bonus.stat));
        }
        h.v.shrine_offer = offer.clone();
        // A bonus never offered is refused.
        let forged = data::bonus_to_wire(&data::Bonus::flat(data::Stat::Might, 9.0));
        assert!(!take_boon(&mut h.v, forged));
        let before = h.stats();
        assert!(take_boon(&mut h.v, data::bonus_to_wire(&offer[1].bonus)));
        assert!(h.shrine_offer.is_empty());
        assert_eq!(h.boons, [offer[1].bonus]);
        assert_ne!(h.stats(), before, "the blessing counts in the stats");
        assert!(!take_boon(&mut h.v, data::bonus_to_wire(&offer[0].bonus)));
    }

    /// One frame of the shrines, on a world holding one hero at `at` and the field's shrines.
    fn shrine_frame(app: &mut App) {
        app.world_mut()
            .run_system_once(
                |mut commands: Commands,
                 mut run: ResMut<Run>,
                 mut rng: ResMut<Rng>,
                 mut field: ResMut<Field>,
                 mut heroes: Query<(&Transform, &mut HeroState), (With<Hero>, Without<Chest>)>,
                 foes: Query<&Enemy>| {
                    let rules = data::Rules::SOLO;
                    tend_shrines(
                        &mut commands,
                        &mut run,
                        &rules,
                        &mut rng,
                        &mut field,
                        0,
                        Vec3::ZERO,
                        0.5,
                        &mut heroes,
                        &foes,
                    );
                },
            )
            .unwrap();
    }

    fn shrine_world(kind: usize, hero_at: Vec3) -> App {
        let mut app = App::new();
        app.insert_resource(Run::default())
            .insert_resource(Rng::seeded(2))
            .insert_resource(Field {
                shrines: vec![Shrine {
                    view: ShrineView {
                        kind,
                        pos: Vec3::ZERO,
                        charge: 0.0,
                        state: ShrineState::Ready,
                    },
                    model: None,
                    foes: Vec::new(),
                }],
                ..Field::default()
            });
        app.world_mut().spawn((
            Transform::from_translation(hero_at),
            hero(),
            Hero::default(),
        ));
        app
    }

    fn the_hero(app: &mut App) -> HeroState {
        let mut q = app.world_mut().query::<&HeroState>();
        let h = q.single(app.world()).unwrap();
        HeroState {
            slot: h.slot,
            v: h.v.clone(),
        }
    }

    #[test]
    fn a_shrine_of_power_charges_while_stood_in_and_offers_its_blessings() {
        let mut app = shrine_world(0, Vec3::X);
        let hold = data::SHRINES[0].hold;
        let frames = (hold / 0.5).ceil() as usize;
        for _ in 0..frames - 1 {
            shrine_frame(&mut app);
        }
        assert!(
            the_hero(&mut app).shrine_offer.is_empty(),
            "not charged yet"
        );
        shrine_frame(&mut app);
        assert_eq!(the_hero(&mut app).shrine_offer.len(), 3);
        let f = app.world().resource::<Field>();
        assert_eq!(f.shrines[0].view.state, ShrineState::Spent);
    }

    #[test]
    fn a_shrine_left_alone_ebbs() {
        let mut app = shrine_world(0, Vec3::X * 50.0);
        app.world_mut().resource_mut::<Field>().shrines[0]
            .view
            .charge = 0.5;
        shrine_frame(&mut app);
        let c = app.world().resource::<Field>().shrines[0].view.charge;
        assert!(c < 0.5 && c > 0.0, "{c}");
    }

    #[test]
    fn greed_pays_every_hero_and_toughens_the_foes() {
        let mut app = shrine_world(2, Vec3::ZERO);
        for _ in 0..4 {
            shrine_frame(&mut app);
        }
        let run = app.world().resource::<Run>();
        assert_eq!(run.greed, 1);
        let tougher = run.threat(data::Rules::SOLO);
        assert!(tougher.health > data::Rules::SOLO.health);
        assert!(tougher.damage > data::Rules::SOLO.damage);
        assert_eq!(the_hero(&mut app).boons, [data::GREED_GOLD]);
    }
}
