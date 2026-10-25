//! The run's field, Megabonk-style: chests to open with the gold the run picks up, and elites
//! standing guard at fixed posts around the battleground, each over a reward of its own (a roll
//! for gear, a spell's rank, a run item, a purse), coming back a while after they fall.
//!
//! Only solo and the host tend the field (it runs in `SimSet`); chests are game objects and
//! elites units, so guests see them as they see everything else. A chest opens for a hero who
//! stands at it with the gold for it, which needs no input of its own over the wire.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::data::{self, Reward};
use super::enemies::{self, Enemy, GemKind};
use super::hero::Hero;
use super::units::{self, flat};
use super::{HeroState, Owned, Rng, Run, Selection, Session};

/// The game-object displays the client's tables have for a chest and a coin, found by model name.
#[derive(Clone, Copy, Default)]
struct Looks {
    chest: Option<u32>,
    coin: Option<u32>,
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
    /// An elite of the [`data::ELITES`] kind.
    Elite(u8),
}

/// One elite post as the marks see it: where, which kind, and how long until it is back (0 while
/// its elite stands). The host's [`Field`] on solo and the host, the host's stream on a guest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PostView {
    pub kind: usize,
    pub pos: Vec3,
    pub wait: f32,
}

/// A chest on the field, and how long a hero has stood at it.
#[derive(Component)]
pub(super) struct Chest {
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
}

/// A spot of the battleground, `t` of the way out to its leash, at angle `a`, on the ground.
fn spot(world: &WorldPoint, center: Vec3, leash: f32, a: f32, t: f32) -> Vec3 {
    let mut p = center + Vec3::new(a.cos(), 0.0, a.sin()) * (leash * t);
    if let Some(h) = world.terrain_height_under(p) {
        p.y = h;
    }
    p
}

fn spawn_chest(commands: &mut Commands, run: &mut Run, pos: Vec3) {
    let (display, scale) = chest_look();
    let e = units::spawn_object(commands, run.guid(), display, scale, pos);
    commands.entity(e).insert((
        Chest {
            hold: 0.0,
            told: false,
        },
        super::Pickup,
        FieldMark::Chest,
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
    let rules = session.rules;

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
            spawn_chest(&mut commands, &mut run, pos);
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
        spawn_chest(&mut commands, &mut run, pos);
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
        field.opened += 1;
        field.returning.push(data::CHEST_RESPAWN_SECS);
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
                    None => grant(&mut hero, Reward::Gold(cost), &mut rng),
                }
            }
            reward => grant(&mut hero, reward, &mut rng),
        };
        run.banner = Some((format!("Chest: {text}"), 4.0));
        // One chest a frame: the next one's price is the new one.
        break;
    }
    run.chest_cost = data::chest_cost(field.opened);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Vitals;

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
}
