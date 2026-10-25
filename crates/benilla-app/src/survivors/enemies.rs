//! The horde: spawning on a ring outside the view, the chase, contact damage, death, and the
//! experience gems the dead leave.

use std::collections::HashMap;

use benilla_world::rig_anim::GlobalSeqDrive;
use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::abilities::Hit;
use super::data::{self, EnemyDef, Rules};
use super::hero::{self, Hero, HERO_RADIUS};
use super::units::{self, flat, yaw_toward, Fx, Marks, UnitSpec, COLOR_HURT};
use super::{HeroState, Hostile, Pickup, Rng, Run, Selection, Session};
use crate::creature_anim::{move_flags, MovementState};
use crate::net::{FieldChanged, Guid, ObjectStore};

#[derive(Component)]
pub(super) struct Enemy {
    pub def: &'static EnemyDef,
    pub hp: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub damage: f32,
    pub radius: f32,
    pub xp: u32,
    pub boss: bool,
    /// Which of the zone's bosses, from 0; its item drops when it falls.
    pub boss_slot: Option<u8>,
    /// The zone's final boss: its fall wins the run.
    pub final_boss: bool,
    /// One of the field's elites ([`data::ELITES`] index): its reward drops when it falls.
    pub elite: Option<u8>,
    /// Where an elite stands guard: it chases only heroes who come within [`GUARD_RADIUS`] of
    /// it, then walks back.
    pub home: Option<Vec3>,
    pub attack_cd: f32,
    pub slow: f32,
    pub slow_t: f32,
    pub root_t: f32,
    pub stun_t: f32,
    pub fear_t: f32,
    pub weaken: f32,
    pub weaken_t: f32,
    pub dots: Vec<Dot>,
    pub dead: bool,
}

pub(super) struct Dot {
    /// The hero who laid it, credited with its ticks.
    pub owner: Entity,
    pub dps: f32,
    pub left: f32,
    pub tick: f32,
}

/// A corpse playing its death before it fades.
#[derive(Component)]
pub(super) struct Dying(f32);

#[derive(Resource, Default)]
pub(super) struct Spawner {
    timer: f32,
}

/// An enemy died here, worth this much.
#[derive(Message, Clone, Copy)]
pub(super) struct Loot {
    pub pos: Vec3,
    pub xp: u32,
    pub boss: bool,
    /// A boss's slot in the zone, for the item it drops.
    pub boss_slot: Option<u8>,
    /// An elite's kind, for its reward.
    pub elite: Option<u8>,
}

#[derive(Clone, Copy)]
pub(super) enum GemKind {
    Xp(u32),
    Heal(f32),
    /// A piece of gear: whoever catches it keeps it.
    Item(&'static data::ItemDef),
    /// The run's gold.
    Gold(u32),
    /// A chest's or an elite's reward, for whoever catches it.
    Reward(data::Reward),
    /// A key, which opens a chest for nothing.
    Key,
}

#[derive(Component)]
pub(super) struct Gem {
    kind: GemKind,
    /// The hero whose magnet caught it, once one has.
    pulled: Option<Entity>,
    speed: f32,
}

/// The `GameObjectDisplayInfo` row and scale a pickup wears: a wisp per experience tier, the
/// bigger tiers rarer, and a turkey leg for health.
fn look_for(kind: GemKind) -> (u32, f32) {
    match kind {
        // A wisp twice the size, purple for the epic and legendary, green below.
        GemKind::Item(i) if i.rarity >= data::Rarity::Epic => (1307, 2.6),
        GemKind::Item(_) => (1267, 2.2),
        GemKind::Gold(_) | GemKind::Reward(data::Reward::Gold(_)) => super::treasure::coin_look(),
        // The rewards a wisp of their own: purple for a spell's rank, green for a run item.
        GemKind::Reward(data::Reward::SpellRank) => (1307, 1.8),
        GemKind::Reward(_) => (1267, 1.8),
        // A key glows gold: the big plain wisp.
        GemKind::Key => (426, 2.2),
        // `TurkeyLeg.mdx`.
        GemKind::Heal(_) => (114, 2.5),
        // `BFD_WispSmallPurple.mdx`.
        GemKind::Xp(xp) if xp >= 20 => (1307, 1.0),
        // `BFD_WispSmallGreen.mdx`.
        GemKind::Xp(xp) if xp >= 5 => (1267, 1.0),
        // `BFD_WispSmall.mdx`.
        GemKind::Xp(_) => (426, 1.0),
    }
}

/// Body radius: the big beasts need more room.
fn radius_for(def: &EnemyDef) -> f32 {
    (if def.big { 1.3 } else { 0.6 }) * def.scale
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_enemy(
    commands: &mut Commands,
    run: &mut Run,
    rules: &Rules,
    def: &'static EnemyDef,
    pos: Vec3,
    hero: Vec3,
    boss_slot: Option<u8>,
    final_boss: bool,
) -> Entity {
    let scale = rules.hp_scale(run.elapsed);
    let serial = run.guid();
    let e = units::spawn_unit(
        commands,
        &UnitSpec {
            serial,
            display: def.display,
            scale: def.scale,
            level: def.level,
            faction: units::FACTION_MONSTER,
            bytes0: 1 << 8,
            weapons: &[],
            sheath: 0,
        },
        pos,
        yaw_toward(hero - pos),
    );
    let hp = def.hp * scale;
    commands.entity(e).insert((
        Enemy {
            def,
            hp,
            max_hp: hp,
            speed: def.speed,
            damage: def.damage * rules.damage_scale(run.elapsed),
            radius: radius_for(def),
            xp: def.xp,
            boss: boss_slot.is_some(),
            boss_slot,
            final_boss,
            elite: None,
            home: None,
            attack_cd: 0.5,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: Vec::new(),
            dead: false,
        },
        Marks::debuffs(),
        Hostile,
    ));
    e
}

fn ground(world: &WorldPoint, mut p: Vec3, fallback: f32) -> Vec3 {
    p.y = world.terrain_height_under(p).unwrap_or(fallback);
    p
}

/// Keep the horde topped up on a ring outside the view of a standing hero, with the bosses and
/// the encircling waves on the clock.
#[allow(clippy::too_many_arguments)]
pub(super) fn spawn_enemies(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    mut spawner: ResMut<Spawner>,
    selection: Res<Selection>,
    session: Res<Session>,
    heroes: Query<(&Transform, &HeroState), With<Hero>>,
    enemies: Query<&Enemy>,
    world: WorldPoint,
) {
    let anchors: Vec<Vec3> = heroes
        .iter()
        .filter(|(_, h)| h.alive())
        .map(|(t, _)| t.translation)
        .collect();
    if anchors.is_empty() {
        return;
    }
    let rules = run.threat(session.rules);
    let anchor = |rng: &mut Rng| anchors[rng.index(anchors.len())];
    let elapsed = run.elapsed;
    let roster = data::roster(selection.map);
    let pool = rules.wave_pool(elapsed, roster.enemies.len());

    let due_bosses = rules.bosses_due(elapsed);
    while run.bosses < due_bosses {
        let (def, final_boss) = rules.boss(roster, run.bosses);
        let slot = Some(run.bosses.min(u32::from(u8::MAX)) as u8);
        let hpos = anchor(&mut rng);
        let a = rng.range(0.0, std::f32::consts::TAU);
        let pos = ground(
            &world,
            hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 30.0,
            hpos.y,
        );
        spawn_enemy(
            &mut commands,
            &mut run,
            &rules,
            def,
            pos,
            hpos,
            slot,
            final_boss,
        );
        run.bosses += 1;
        run.banner = Some(if final_boss {
            (format!("{} has come: bring them down!", def.name), 5.0)
        } else {
            (format!("{} approaches!", def.name), 4.0)
        });
    }

    // Half way between bosses, a closing ring of the moment's weakest around every standing hero.
    let due_rings = ((elapsed + rules.ambush_every / 2.0) / rules.ambush_every.max(1.0)) as u32;
    if run.rings < due_rings {
        run.rings = due_rings;
        let def = &roster.enemies[pool[0]];
        let n = ((28 + (rules.progress(elapsed) * 60.0) as usize) as f32 * rules.density).max(8.0)
            as usize;
        let n = (n / anchors.len()).max(8);
        for &hpos in &anchors {
            for i in 0..n {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                let pos = ground(
                    &world,
                    hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 26.0,
                    hpos.y,
                );
                spawn_enemy(&mut commands, &mut run, &rules, def, pos, hpos, None, false);
            }
        }
        run.banner = Some(("They're surrounding you!".into(), 3.0));
    }

    spawner.timer -= time.delta_secs();
    if spawner.timer > 0.0 {
        return;
    }
    spawner.timer = 0.12;
    let alive = enemies.iter().filter(|e| !e.dead).count();
    let target = rules.target_alive(elapsed);
    if alive >= target {
        return;
    }
    let n = (((target - alive) as f32) / 6.0)
        .ceil()
        .min(4.0 * anchors.len() as f32) as usize;
    for _ in 0..n {
        let def = &roster.enemies[pool[rng.index(pool.len())]];
        let hpos = anchor(&mut rng);
        let a = rng.range(0.0, std::f32::consts::TAU);
        let r = rng.range(34.0, 42.0);
        let pos = ground(&world, hpos + Vec3::new(a.cos(), 0.0, a.sin()) * r, hpos.y);
        spawn_enemy(&mut commands, &mut run, &rules, def, pos, hpos, None, false);
    }
}

/// A hero the horde can see: where, and whether a Fade hides them from all but bosses.
#[derive(Clone, Copy)]
struct Quarry {
    e: Entity,
    pos: Vec3,
    vanished: bool,
}

/// The chase: straight at the nearest hero in sight, pushed apart from each other, stopped by
/// crowd control, and swinging at whoever is in reach.
#[allow(clippy::type_complexity)]
pub(super) fn move_enemies(
    time: Res<Time>,
    world: WorldPoint,
    mut heroes: Query<(Entity, &Transform, &mut HeroState), (With<Hero>, Without<Enemy>)>,
    mut enemies: Query<(Entity, &mut Transform, &mut Enemy, &mut MovementState), Without<Hero>>,
    mut fx: Fx,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let quarry: Vec<Quarry> = heroes
        .iter()
        .filter(|(_, _, h)| h.alive())
        .map(|(e, t, h)| Quarry {
            e,
            pos: t.translation,
            vanished: h.mobile.vanish_t > 0.0,
        })
        .collect();

    // Separation over a coarse grid: only neighbours in the same or adjacent cells push.
    const CELL: f32 = 3.0;
    let bodies: Vec<(Entity, Vec3, f32)> = enemies
        .iter()
        .filter(|(_, _, e, _)| !e.dead)
        .map(|(ent, tf, e, _)| (ent, tf.translation, e.radius))
        .collect();
    let key = |p: Vec3| ((p.x / CELL).floor() as i32, (p.z / CELL).floor() as i32);
    let mut grid: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, b) in bodies.iter().enumerate() {
        grid.entry(key(b.1)).or_default().push(i);
    }
    let mut push: HashMap<Entity, Vec3> = HashMap::with_capacity(bodies.len());
    for (i, &(ent, pos, r)) in bodies.iter().enumerate() {
        let (cx, cz) = key(pos);
        let mut acc = Vec3::ZERO;
        for dx in -1..=1 {
            for dz in -1..=1 {
                let Some(cell) = grid.get(&(cx + dx, cz + dz)) else {
                    continue;
                };
                for &j in cell {
                    if j == i {
                        continue;
                    }
                    let (_, other, or) = bodies[j];
                    let d = flat(pos - other);
                    let len = d.length();
                    let min = r + or;
                    if len < min && len > 1e-4 {
                        acc += d / len * (min - len);
                    } else if len <= 1e-4 {
                        acc += Vec3::new((i as f32).sin(), 0.0, (i as f32).cos()) * 0.1;
                    }
                }
            }
        }
        push.insert(ent, acc);
    }

    for (ent, mut tf, mut e, mut mv) in &mut enemies {
        let e = &mut *e;
        if e.dead {
            if mv.speed != 0.0 || mv.flags != 0 {
                mv.speed = 0.0;
                mv.flags = 0;
            }
            continue;
        }
        e.attack_cd -= dt;
        for t in [
            &mut e.slow_t,
            &mut e.root_t,
            &mut e.stun_t,
            &mut e.fear_t,
            &mut e.weaken_t,
        ] {
            *t = (*t - dt).max(0.0);
        }
        if e.slow_t <= 0.0 {
            e.slow = 1.0;
        }
        if e.weaken_t <= 0.0 {
            e.weaken = 1.0;
        }
        let pos = tf.translation;
        let near = |q: &&Quarry| flat(q.pos - pos).length();
        // The nearest standing hero at all, and the nearest this enemy can see.
        let nearest = quarry.iter().min_by(|a, b| near(a).total_cmp(&near(b)));
        let mut seen = quarry
            .iter()
            .filter(|q| e.boss || !q.vanished)
            .min_by(|a, b| near(a).total_cmp(&near(b)));
        // A guard minds only who comes near its post.
        if let Some(home) = e.home {
            if seen.is_some_and(|q| flat(q.pos - home).length() > GUARD_RADIUS) {
                seen = None;
            }
        }

        // Strays far from every hero are brought back in front of the nearest, as the reference
        // genre does.
        if let Some(n) = nearest.filter(|_| e.home.is_none()) {
            let to = flat(n.pos - pos);
            if to.length() > 75.0 {
                let back = n.pos + to.normalize_or_zero() * 38.0;
                tf.translation = ground(&world, back, n.pos.y);
                continue;
            }
        }

        // Under Fade, or with every hero down, the horde mills where it stands.
        let held = e.stun_t > 0.0 || e.root_t > 0.0 || seen.is_none();
        let (to_hero, contact) = match seen {
            Some(q) => {
                let to = flat(q.pos - pos);
                (to, to.length() <= HERO_RADIUS + e.radius + 0.35)
            }
            None => (Vec3::ZERO, false),
        };
        // A guard left alone walks back to its post.
        let (held, to_hero, contact) = match e.home {
            Some(home) if seen.is_none() && e.stun_t <= 0.0 && e.root_t <= 0.0 => {
                let back = flat(home - pos);
                if back.length() > 1.0 {
                    (false, back, false)
                } else {
                    (true, Vec3::ZERO, false)
                }
            }
            _ => (held, to_hero, contact),
        };
        let mut step = Vec3::ZERO;
        let mut speed = 0.0;
        if !held {
            let dir = if e.fear_t > 0.0 {
                -to_hero.normalize_or_zero()
            } else {
                to_hero.normalize_or_zero()
            };
            if !contact || e.fear_t > 0.0 {
                speed = e.speed * e.slow;
                step = dir * speed;
            }
            if dir != Vec3::ZERO {
                let target = Quat::from_rotation_y(yaw_toward(dir));
                tf.rotation = tf.rotation.slerp(target, (dt * 10.0).min(1.0));
            }
        }
        let shove = push.get(&ent).copied().unwrap_or_default();
        let mut next = pos + step * dt + shove * (dt * 8.0).min(0.5);
        next = ground(&world, next, pos.y);
        tf.translation = next;
        if speed > 0.0 {
            mv.speed = speed;
            mv.flags = move_flags::FORWARD;
        } else if mv.speed != 0.0 || mv.flags != 0 {
            mv.speed = 0.0;
            mv.flags = 0;
        }

        let Some(q) = seen else { continue };
        if contact && e.stun_t <= 0.0 && e.fear_t <= 0.0 && e.attack_cd <= 0.0 {
            let Ok((hero_e, _, mut hero)) = heroes.get_mut(q.e) else {
                continue;
            };
            e.attack_cd = if e.boss { 1.0 } else { 1.3 };
            let raw = e.damage * e.weaken;
            let taken = hero::hurt_hero(&mut hero.v, raw);
            fx.swing(ent, hero_e, taken);
            if taken > 0.0 {
                fx.number(hero_e, taken, false, COLOR_HURT);
            }
            if hero.thorns_t > 0.0 {
                hits.write(Hit::direct(
                    hero_e,
                    ent,
                    hero.thorns * hero.stats().might,
                    false,
                ));
            }
            // A run item's thorns return a share of every blow, ability or not.
            let thorns: f32 = hero
                .item_effects()
                .map(|e| match e {
                    data::ItemEffect::Thorns(d) => d,
                    _ => 0.0,
                })
                .sum();
            if thorns > 0.0 && taken > 0.0 {
                hits.write(Hit {
                    proc: true,
                    color: super::units::COLOR_SPELL,
                    ..Hit::direct(hero_e, ent, thorns * hero.stats().might, false)
                });
            }
        }
    }
}

/// Mark timers run down; the state kits reap with them.
pub(super) fn tick_marks(
    time: Res<Time>,
    mut units: Query<(Entity, &Guid, &mut ObjectStore, &mut Marks), With<Enemy>>,
    mut fields: MessageWriter<FieldChanged>,
) {
    let dt = time.delta_secs();
    for (entity, guid, mut store, mut marks) in &mut units {
        marks.tick(dt);
        marks.flush(entity, guid.0, &mut store, &mut fields);
    }
}

/// Corpses fade once their death has played.
pub(super) fn reap_dead(
    mut commands: Commands,
    time: Res<Time>,
    mut dying: Query<(Entity, &mut Dying)>,
) {
    for (e, mut d) in &mut dying {
        d.0 += time.delta_secs();
        if d.0 > 2.5 {
            commands
                .entity(e)
                .remove::<Dying>()
                .insert(benilla_world::model_fade::DespawnFade::default());
        }
    }
}

pub(super) fn mark_dying(commands: &mut Commands, e: Entity) {
    commands.entity(e).insert(Dying(0.0));
}

/// How far from its post an elite goes after a hero.
pub(super) const GUARD_RADIUS: f32 = 18.0;

/// Most gems on the ground at once; past it, a kill's worth joins an existing gem.
const MAX_GEMS: usize = 350;

pub(super) fn drop_loot(
    mut commands: Commands,
    mut drops: MessageReader<Loot>,
    mut rng: ResMut<Rng>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    mut gems: Query<&mut Gem>,
) {
    let mut count = gems.iter().count();
    for d in drops.read() {
        // A boss drops its own item; one of the horde, rarely, one of the zone's.
        let item = match d.boss_slot {
            Some(k) => data::boss_item(selection.map, usize::from(k)),
            None => data::horde_item(selection.map, rng.f32()),
        };
        if let Some(item) = item {
            spawn_gem(
                &mut commands,
                run.guid(),
                d.pos - Vec3::X * 0.8,
                GemKind::Item(item),
            );
            run.banner = Some((format!("{} dropped!", item.name), 4.0));
        }
        // The run's gold: a coin now and then from the horde, a pile from a boss, a purse from an
        // elite with what it guards.
        let gold = if d.boss {
            data::BOSS_GOLD
        } else if d.elite.is_some() {
            data::ELITE_GOLD
        } else if rng.index(data::COIN_ODDS as usize) == 0 {
            1
        } else {
            0
        };
        if gold > 0 {
            spawn_gem(
                &mut commands,
                run.guid(),
                d.pos + Vec3::Z * 0.8,
                GemKind::Gold(gold),
            );
        }
        if let Some(k) = d.elite {
            if rng.f32() < data::KEY_ODDS {
                spawn_gem(
                    &mut commands,
                    run.guid(),
                    d.pos + Vec3::X * 0.8,
                    GemKind::Key,
                );
            }
            let kind = &data::ELITES[usize::from(k).min(data::ELITES.len() - 1)];
            let at = d.pos - Vec3::Z * 0.8;
            match kind.reward {
                data::Reward::Gear { percent } => {
                    let gear = (rng.index(100) < usize::from(percent))
                        .then(|| {
                            data::horde_item(
                                selection.map,
                                rng.f32() / data::HORDE_ITEM_ODDS as f32,
                            )
                        })
                        .flatten();
                    match gear {
                        Some(item) => {
                            spawn_gem(&mut commands, run.guid(), at, GemKind::Item(item));
                            run.banner = Some((format!("{} dropped!", item.name), 4.0));
                        }
                        None => spawn_gem(
                            &mut commands,
                            run.guid(),
                            at,
                            GemKind::Gold(data::ELITE_GOLD * 3),
                        ),
                    }
                }
                reward => spawn_gem(&mut commands, run.guid(), at, GemKind::Reward(reward)),
            }
        }
        if d.boss || rng.f32() < 0.015 {
            let heal = GemKind::Heal(if d.boss { 1000.0 } else { 30.0 });
            spawn_gem(&mut commands, run.guid(), d.pos + Vec3::X * 0.8, heal);
        }
        if count >= MAX_GEMS && !d.boss {
            if let Some(mut g) = gems.iter_mut().find(|g| matches!(g.kind, GemKind::Xp(_))) {
                if let GemKind::Xp(xp) = &mut g.kind {
                    *xp += d.xp;
                }
                continue;
            }
        }
        spawn_gem(&mut commands, run.guid(), d.pos, GemKind::Xp(d.xp));
        count += 1;
    }
}

pub(super) fn spawn_gem(commands: &mut Commands, serial: u64, pos: Vec3, kind: GemKind) {
    let (display, scale) = look_for(kind);
    let e = units::spawn_object(commands, serial, display, scale, pos + Vec3::Y * 0.5);
    commands.entity(e).insert((
        Gem {
            kind,
            pulled: None,
            speed: 6.0,
        },
        Pickup,
    ));
}

/// Gems in a standing hero's magnet fly to that hero; experience is shared or kept by the rules,
/// and levels bank as it lands.
#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(super) fn collect_gems(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    session: Res<Session>,
    mut heroes: Query<(Entity, &Transform, &mut HeroState), (With<Hero>, Without<Gem>)>,
    mut gems: Query<(Entity, &mut Transform, &mut Gem)>,
) {
    let dt = time.delta_secs();
    let catchers: Vec<(Entity, Vec3, f32)> = heroes
        .iter()
        .filter(|(_, _, h)| h.alive())
        .map(|(e, t, h)| (e, t.translation, h.stats().magnet))
        .collect();
    if catchers.is_empty() {
        return;
    }
    let mut banked: Vec<(Entity, GemKind)> = Vec::new();
    for (e, mut tf, mut g) in &mut gems {
        if g.pulled.is_none() {
            g.pulled = catchers
                .iter()
                .filter(|(_, p, magnet)| flat(*p - tf.translation).length() <= *magnet)
                .min_by(|a, b| {
                    flat(a.1 - tf.translation)
                        .length()
                        .total_cmp(&flat(b.1 - tf.translation).length())
                })
                .map(|c| c.0);
        }
        let Some(to) = g.pulled else { continue };
        // The catcher fell or left: the gem waits for the next magnet.
        let Some(&(_, at, _)) = catchers.iter().find(|c| c.0 == to) else {
            g.pulled = None;
            g.speed = 6.0;
            continue;
        };
        let target = at + Vec3::Y * 1.0;
        let d = target - tf.translation;
        let dist = d.length();
        if dist < 0.9 {
            banked.push((to, g.kind));
            commands.entity(e).despawn();
            continue;
        }
        g.speed += 30.0 * dt;
        tf.translation += d / dist * (g.speed * dt).min(dist);
    }

    for (catcher, kind) in banked {
        match kind {
            GemKind::Xp(xp) => {
                for (e, _, mut h) in &mut heroes {
                    if e == catcher || (session.rules.shared_xp && h.alive()) {
                        let growth = h.stats().growth;
                        h.xp += xp as f32 * data::XP_RATE * growth;
                    }
                }
            }
            GemKind::Heal(hp) => {
                if let Ok((_, _, mut h)) = heroes.get_mut(catcher) {
                    let max = h.stats().max_hp;
                    h.hp = (h.hp + hp).min(max);
                }
            }
            GemKind::Gold(n) => {
                if let Ok((_, _, mut h)) = heroes.get_mut(catcher) {
                    let mult = h.stats().gold;
                    h.gold += n as f32 * mult;
                }
            }
            GemKind::Key => {
                if let Ok((_, _, mut h)) = heroes.get_mut(catcher) {
                    h.keys = (h.keys + 1).min(data::MAX_KEYS);
                    h.version += 1;
                    if h.slot == session.slot {
                        run.banner = Some(("A key! It opens a chest for nothing.".into(), 3.0));
                    }
                }
            }
            GemKind::Reward(reward) => {
                if let Ok((_, _, mut h)) = heroes.get_mut(catcher) {
                    let text = super::treasure::grant(&mut h, reward, &mut rng);
                    run.banner = Some((text, 4.0));
                }
            }
            GemKind::Item(item) => {
                if let Ok((_, _, mut h)) = heroes.get_mut(catcher) {
                    h.loot.push(item);
                    if h.slot == session.slot {
                        run.banner = Some((
                            format!("You found {} ({})", item.name, item.rarity.name()),
                            4.0,
                        ));
                    }
                }
            }
        }
    }

    let mut learned: Option<String> = None;
    for (_, _, mut h) in &mut heroes {
        while h.xp >= data::xp_to_next(h.level) {
            let need = data::xp_to_next(h.level);
            h.xp -= need;
            h.level += 1;
            h.pending_levels += 1;
            super::offer::earn_tokens(&mut h.v);
            if h.level == super::mobility::LEARN_LEVEL && h.slot == session.slot {
                if let Some(m) = super::mobility::for_class(h.class.id) {
                    learned = Some(format!("You have learned {}! Press F.", m.name));
                }
            }
        }
    }
    if let Some(text) = learned {
        run.banner = Some((text, 4.0));
    }
}

/// Pickups hang still: a wisp's own loop (its circling motes and their trails) stops on its first
/// frame, clip and global sequences alike, as soon as the model attaches.
pub(super) fn still_gems(
    mut players: Query<&mut AnimationPlayer, (With<Pickup>, Added<AnimationPlayer>)>,
    mut drives: Query<&mut GlobalSeqDrive, (With<Pickup>, Added<GlobalSeqDrive>)>,
) {
    for mut player in &mut players {
        for (_, active) in player.playing_animations_mut() {
            active.set_speed(0.0);
        }
    }
    for mut drive in &mut drives {
        drive.set_paused(true);
    }
}
