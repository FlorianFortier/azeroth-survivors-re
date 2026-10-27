//! Auto-cast: every owned ability fires on its own cooldown at the targets its kind picks, with
//! the spell's own visuals; damage lands when its missile would.

use bevy::prelude::*;

use super::data::{self, AbilityDef, Effect, ItemEffect, Kind};
use super::enemies::{self, Enemy, Loot};
use super::hero::{self, Hero};
use super::units::{self, flat, Fx, Marks, UnitSpec, COLOR_HEAL, COLOR_MELEE, COLOR_SPELL};
use super::{HeroState, LocalHero, Rng, Run, RunEntity, Vitals};
use crate::net::{FieldChanged, Guid, ObjectStore};

/// One blow landing on one enemy.
#[derive(Message, Clone)]
pub(super) struct Hit {
    /// The hero whose spell, totem or thorns this is: credited with the damage and the kill.
    pub owner: Entity,
    pub target: Entity,
    pub damage: f32,
    pub dot: f32,
    pub dot_dur: f32,
    pub effect: Effect,
    pub mark: u32,
    pub mark_dur: f32,
    /// Also hits everything this close to the target, for less.
    pub splash: f32,
    pub crit: bool,
    pub color: u32,
    /// A run item's arc or burst: it fires no item effect of its own, so procs never chain.
    pub proc: bool,
    /// The spell it is credited to on the damage meter, 0 for none.
    pub spell: u32,
}

impl Hit {
    pub fn direct(owner: Entity, target: Entity, damage: f32, crit: bool) -> Self {
        Self {
            owner,
            target,
            damage,
            dot: 0.0,
            dot_dur: 0.0,
            effect: Effect::None,
            mark: 0,
            mark_dur: 0.0,
            splash: 0.0,
            crit,
            color: COLOR_MELEE,
            proc: false,
            spell: 0,
        }
    }
}

/// Hits in flight, by the run time they land.
#[derive(Resource, Default)]
pub(super) struct PendingHits(pub Vec<(f32, Hit)>);

/// A ground spell's area, ticking on whatever stands in it.
#[derive(Component)]
pub(super) struct GroundArea {
    owner: Entity,
    damage: f32,
    dot: f32,
    dot_dur: f32,
    radius: f32,
    left: f32,
    every: f32,
    next_tick: f32,
    effect: Effect,
    mark: u32,
    /// The spell whose impact plays on each enemy a tick hits, 0 for none.
    impact: u32,
    /// The spell it is credited to.
    spell: u32,
}

#[derive(Component)]
pub(super) struct Totem {
    owner: Entity,
    cast: u32,
    damage: f32,
    range: f32,
    radius: f32,
    left: f32,
    tick: f32,
    pulse: bool,
}

#[derive(Clone, Copy)]
struct Foe {
    e: Entity,
    pos: Vec3,
    hp: f32,
    dist: f32,
    radius: f32,
}

/// The numbers one cast uses, from the ability's level and the hero's stats.
struct Params {
    dmg: f32,
    dot: f32,
    count: usize,
    radius: f32,
    duration: f32,
    cooldown: f32,
}

fn mark_secs(def: &AbilityDef, duration: f32) -> f32 {
    match def.effect {
        Effect::Slow(_, t) | Effect::Weaken(_, t) => t,
        Effect::Root(t) | Effect::Stun(t) | Effect::Fear(t) => t,
        _ if duration > 0.0 => duration,
        _ => 3.0,
    }
}

fn hit_for(
    owner: Entity,
    def: &AbilityDef,
    p: &Params,
    target: Entity,
    damage: f32,
    crit: bool,
) -> Hit {
    Hit {
        owner,
        target,
        damage,
        dot: p.dot,
        dot_dur: p.duration,
        effect: def.effect,
        mark: def.mark,
        mark_dur: mark_secs(def, p.duration),
        splash: if matches!(def.kind, Kind::Bolt | Kind::Strike) {
            p.radius
        } else {
            0.0
        },
        crit,
        color: if def.kind == Kind::Melee {
            COLOR_MELEE
        } else {
            COLOR_SPELL
        },
        proc: false,
        spell: def.spell,
    }
}

fn schedule(
    pending: &mut PendingHits,
    hits: &mut MessageWriter<Hit>,
    now: f32,
    delay: f32,
    hit: Hit,
) {
    if delay <= 0.0 {
        hits.write(hit);
    } else {
        pending.0.push((now + delay, hit));
    }
}

fn roll(rng: &mut Rng, crit: f32, dmg: f32) -> (f32, bool) {
    if rng.f32() < crit {
        (dmg * 2.0, true)
    } else {
        (dmg, false)
    }
}

fn nearest(foes: &[Foe], range: f32, n: usize) -> Vec<Foe> {
    foes.iter()
        .filter(|f| f.dist <= range + f.radius)
        .take(n)
        .copied()
        .collect()
}

/// The foes nearest first, but the hero's `target` first of all: a spell that strikes the nearest
/// takes the target while it is within its reach, and the next nearest otherwise.
fn aimed_order(foes: &[Foe], target: Option<Entity>) -> Vec<Foe> {
    let mut v = foes.to_vec();
    if let Some(i) = target.and_then(|t| v.iter().position(|f| f.e == t)) {
        let picked = v.remove(i);
        v.insert(0, picked);
    }
    v
}

fn toughest(foes: &[Foe], range: f32, n: usize) -> Vec<Foe> {
    let mut v: Vec<Foe> = foes
        .iter()
        .filter(|f| f.dist <= range + f.radius)
        .copied()
        .collect();
    v.sort_by(|a, b| b.hp.total_cmp(&a.hp));
    v.truncate(n);
    v
}

fn around(foes: &[Foe], center: Vec3, r: f32) -> Vec<Foe> {
    foes.iter()
        .filter(|f| flat(f.pos - center).length() <= r + f.radius * 0.5)
        .copied()
        .collect()
}

/// The point among the nearest enemies in range with the most others within `r` of it.
fn densest(foes: &[Foe], range: f32, r: f32) -> Option<Vec3> {
    let near: Vec<&Foe> = foes.iter().filter(|f| f.dist <= range).take(40).collect();
    near.iter()
        .map(|f| {
            (
                f.pos,
                near.iter()
                    .filter(|o| flat(o.pos - f.pos).length() <= r)
                    .count(),
            )
        })
        .max_by_key(|&(_, n)| n)
        .map(|(p, _)| p)
}

fn entities(foes: &[Foe]) -> Vec<Entity> {
    foes.iter().map(|f| f.e).collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn cast_abilities(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut rng: ResMut<Rng>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut heroes: Query<(Entity, &Transform, &mut HeroState, Has<LocalHero>), With<Hero>>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    target: Option<Res<super::targeting::HeroTarget>>,
    mut fx: Fx,
    mut pending: ResMut<PendingHits>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let now = run.elapsed;
    let live: Vec<(Entity, Vec3, f32, f32)> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead)
        .map(|(e, tf, en)| (e, tf.translation, en.hp, en.radius))
        .collect();
    let speed_of = |spell: u32| {
        spells
            .as_deref()
            .and_then(|s| s.catalog.get(spell))
            .map_or(0.0, |d| d.speed)
    };

    let target = target.and_then(|t| t.0);
    for (me, htf, mut hero, local) in &mut heroes {
        if !hero.alive() {
            continue;
        }
        let hero = &mut hero.v;
        let hpos = htf.translation;
        let stats = hero.stats();
        let mut foes: Vec<Foe> = live
            .iter()
            .map(|&(e, pos, hp, radius)| Foe {
                e,
                pos,
                hp,
                dist: hero::reach(hpos, pos),
                radius,
            })
            .collect();
        foes.sort_by(|a, b| a.dist.total_cmp(&b.dist));
        let aimed = aimed_order(&foes, target.filter(|_| local));
        cast_for(
            me,
            hpos,
            hero,
            &stats,
            &foes,
            &aimed,
            now,
            dt,
            &speed_of,
            &mut commands,
            &mut run,
            &mut rng,
            &mut fx,
            &mut pending,
            &mut hits,
        );
    }
}

/// One hero's abilities that are off cooldown fire at the targets their kinds pick.
#[allow(clippy::too_many_arguments)]
fn cast_for(
    me: Entity,
    hpos: Vec3,
    hero: &mut Vitals,
    stats: &super::Stats,
    foes: &[Foe],
    aimed: &[Foe],
    now: f32,
    dt: f32,
    speed_of: &dyn Fn(u32) -> f32,
    commands: &mut Commands,
    run: &mut Run,
    rng: &mut Rng,
    fx: &mut Fx,
    pending: &mut PendingHits,
    hits: &mut MessageWriter<Hit>,
) {
    let stats = *stats;
    for i in 0..hero.abilities.len() {
        hero.abilities[i].cd -= dt;
        if hero.abilities[i].cd > 0.0 {
            continue;
        }
        let o = &hero.abilities[i];
        let def = o.def;
        let mult = o.damage_mult() * stats.might;
        let p = Params {
            dmg: def.damage * mult,
            dot: def.dot * mult,
            count: o.count() as usize,
            radius: o.radius(&stats),
            duration: o.duration(),
            cooldown: o.cooldown(&stats),
        };
        let speed = speed_of(def.spell);
        let fired = match def.kind {
            Kind::Bolt | Kind::Strike | Kind::Melee => {
                let reach = if def.kind == Kind::Melee {
                    def.range + 0.5
                } else {
                    def.range
                };
                let targets = if matches!(
                    def.key,
                    "pyroblast" | "aimed_shot" | "exorcism" | "starfire"
                ) {
                    toughest(foes, reach, p.count)
                } else {
                    nearest(aimed, reach, p.count)
                };
                if targets.is_empty() {
                    false
                } else {
                    if def.kind == Kind::Melee {
                        fx.swing(me, targets[0].e, p.dmg);
                        for f in &targets {
                            fx.impact(f.e, def.spell);
                        }
                    } else {
                        fx.cast(me, def.spell, &entities(&targets), None);
                    }
                    for f in &targets {
                        let (dmg, crit) = roll(rng, stats.crit, p.dmg);
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        schedule(
                            pending,
                            hits,
                            now,
                            delay,
                            hit_for(me, def, &p, f.e, dmg, crit),
                        );
                    }
                    true
                }
            }
            Kind::Nova | Kind::Cone => {
                let mut targets = around(foes, hpos, p.radius);
                if def.kind == Kind::Cone {
                    if let Some(first) = targets.first().copied() {
                        let aim = flat(first.pos - hpos).normalize_or_zero();
                        targets.retain(|f| {
                            let d = flat(f.pos - hpos);
                            d.length() < 2.0 || d.normalize_or_zero().dot(aim) >= 0.8
                        });
                    }
                }
                if targets.is_empty() {
                    false
                } else {
                    fx.cast(me, def.spell, &entities(&targets), None);
                    for f in &targets {
                        let (dmg, crit) = roll(rng, stats.crit, p.dmg);
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        schedule(
                            pending,
                            hits,
                            now,
                            delay,
                            hit_for(me, def, &p, f.e, dmg, crit),
                        );
                    }
                    true
                }
            }
            Kind::Ground | Kind::GroundSelf => {
                let center = if def.kind == Kind::GroundSelf {
                    foes.first()
                        .filter(|f| f.dist <= p.radius + 4.0)
                        .map(|_| hpos)
                } else {
                    densest(foes, def.range, p.radius)
                };
                match center {
                    None => false,
                    Some(center) => {
                        fx.cast(me, def.spell, &[], Some(center));
                        let area = if def.overhead {
                            commands
                                .spawn((Transform::from_translation(center), RunEntity))
                                .id()
                        } else {
                            let serial = run.guid();
                            units::spawn_area_object(commands, serial, def.spell, p.radius, center)
                        };
                        let burst = def.duration <= 0.0;
                        commands.entity(area).insert(GroundArea {
                            owner: me,
                            damage: p.dmg,
                            dot: p.dot,
                            dot_dur: if burst { 5.0 } else { 0.0 },
                            radius: p.radius,
                            left: if burst { 1.5 } else { p.duration },
                            every: if burst { f32::INFINITY } else { 1.0 },
                            next_tick: if burst { 0.0 } else { 0.4 },
                            effect: def.effect,
                            mark: def.mark,
                            impact: if def.overhead { def.spell } else { 0 },
                            spell: def.spell,
                        });
                        true
                    }
                }
            }
            Kind::Chain => match nearest(aimed, def.range, 1).first().copied() {
                None => false,
                Some(first) => {
                    let mut chain = vec![first];
                    while chain.len() < p.count + 1 {
                        let last = chain[chain.len() - 1].pos;
                        let next = foes
                            .iter()
                            .filter(|f| !chain.iter().any(|c| c.e == f.e))
                            .filter(|f| flat(f.pos - last).length() <= p.radius)
                            .min_by(|a, b| {
                                flat(a.pos - last)
                                    .length()
                                    .total_cmp(&flat(b.pos - last).length())
                            });
                        match next {
                            Some(f) => chain.push(*f),
                            None => break,
                        }
                    }
                    fx.cast(me, def.spell, &entities(&chain), None);
                    for (k, f) in chain.iter().enumerate() {
                        let (dmg, crit) = roll(rng, stats.crit, p.dmg * 0.85f32.powi(k as i32));
                        schedule(
                            pending,
                            hits,
                            now,
                            0.08 * k as f32,
                            hit_for(me, def, &p, f.e, dmg, crit),
                        );
                    }
                    true
                }
            },
            Kind::Dot => {
                let mut pool: Vec<Foe> = foes
                    .iter()
                    .filter(|f| f.dist <= def.range)
                    .take(p.count * 3)
                    .copied()
                    .collect();
                if pool.is_empty() {
                    false
                } else {
                    let mut targets = Vec::with_capacity(p.count);
                    while targets.len() < p.count && !pool.is_empty() {
                        targets.push(pool.swap_remove(rng.index(pool.len())));
                    }
                    fx.cast(me, def.spell, &entities(&targets), None);
                    for f in &targets {
                        let delay = if speed > 0.0 { f.dist / speed } else { 0.0 };
                        schedule(
                            pending,
                            hits,
                            now,
                            delay,
                            hit_for(me, def, &p, f.e, 0.0, false),
                        );
                    }
                    true
                }
            }
            Kind::Aura => {
                let targets = around(foes, hpos, p.radius);
                if targets.is_empty() {
                    false
                } else {
                    for f in targets.iter().take(6) {
                        fx.impact(f.e, def.spell);
                    }
                    for f in &targets {
                        let (dmg, crit) = roll(rng, stats.crit, p.dmg);
                        hits.write(hit_for(me, def, &p, f.e, dmg, crit));
                    }
                    true
                }
            }
            Kind::Orbit => {
                let targets = nearest(foes, p.radius, p.count);
                if targets.is_empty() {
                    false
                } else {
                    for f in &targets {
                        fx.impact(f.e, 403);
                        let (dmg, crit) = roll(rng, stats.crit, p.dmg);
                        hits.write(hit_for(me, def, &p, f.e, dmg, crit));
                    }
                    true
                }
            }
            Kind::Shield => {
                hero.shield = p.dmg;
                hero.shield_t = p.duration;
                fx.cast(me, def.spell, &[me], None);
                true
            }
            Kind::Immune => {
                if foes.first().is_some_and(|f| f.dist <= 5.0) {
                    hero.immune_t = p.duration;
                    fx.cast(me, def.spell, &[me], None);
                    true
                } else {
                    false
                }
            }
            Kind::Thorns => {
                if foes.first().is_some_and(|f| f.dist <= 8.0) {
                    hero.thorns = p.dmg;
                    hero.thorns_t = p.duration;
                    hero.thorns_spell = def.spell;
                    fx.cast(me, def.spell, &[me], None);
                    true
                } else {
                    false
                }
            }
            Kind::Heal => {
                if hero.hp < stats.max_hp * 0.85 {
                    hero.heal_rate = p.dmg / p.duration.max(1.0);
                    hero.heal_t = p.duration;
                    fx.cast(me, def.spell, &[me], None);
                    fx.number(me, p.dmg, false, COLOR_HEAL);
                    true
                } else {
                    false
                }
            }
            Kind::Totem => {
                if foes
                    .first()
                    .is_some_and(|f| f.dist <= def.range.max(p.radius) + 6.0)
                {
                    fx.cast(me, def.spell, &[], None);
                    let a = rng.range(0.0, std::f32::consts::TAU);
                    let pos = hpos + Vec3::new(a.cos(), 0.0, a.sin()) * 2.0;
                    let serial = run.guid();
                    let totem = units::spawn_unit(
                        commands,
                        &UnitSpec {
                            serial,
                            display: def.totem.0,
                            scale: 1.0,
                            level: 60,
                            faction: units::FACTION_FRIENDLY,
                            bytes0: 0,
                            weapons: &[],
                            sheath: 0,
                        },
                        pos,
                        0.0,
                    );
                    commands.entity(totem).insert(Totem {
                        owner: me,
                        cast: def.totem.1,
                        damage: p.dmg,
                        range: def.range,
                        radius: p.radius,
                        left: p.duration,
                        tick: 0.3,
                        pulse: def.key == "magma_totem",
                    });
                    true
                } else {
                    false
                }
            }
        };
        hero.abilities[i].cd = if fired { p.cooldown } else { 0.2 };
    }
}

/// Ground areas tick, and totems fight, until they run out.
#[allow(clippy::type_complexity)]
pub(super) fn tick_areas(
    mut commands: Commands,
    time: Res<Time>,
    run: Res<Run>,
    mut rng: ResMut<Rng>,
    spells: Option<Res<crate::ui_action::Spells>>,
    heroes: Query<&HeroState>,
    mut areas: Query<(Entity, &Transform, &mut GroundArea)>,
    mut totems: Query<(Entity, &Transform, &mut Totem), Without<GroundArea>>,
    enemies: Query<(Entity, &Transform, &Enemy), (Without<GroundArea>, Without<Totem>)>,
    mut fx: Fx,
    mut pending: ResMut<PendingHits>,
    mut hits: MessageWriter<Hit>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    // A hero who left mid-run leaves areas behind at the base numbers.
    let stats_of = |owner: Entity| {
        heroes
            .get(owner)
            .map_or_else(|_| Vitals::default().stats(), |h| h.stats())
    };
    let now = run.elapsed;
    let live: Vec<Foe> = enemies
        .iter()
        .filter(|(_, _, e)| !e.dead)
        .map(|(e, tf, en)| Foe {
            e,
            pos: tf.translation,
            hp: en.hp,
            dist: 0.0,
            radius: en.radius,
        })
        .collect();

    for (e, tf, mut area) in &mut areas {
        area.left -= dt;
        area.next_tick -= dt;
        if area.next_tick <= 0.0 {
            area.next_tick += area.every;
            let stats = stats_of(area.owner);
            for (i, f) in around(&live, tf.translation, area.radius)
                .into_iter()
                .enumerate()
            {
                if area.impact != 0 && i < 12 {
                    fx.impact(f.e, area.impact);
                }
                let (dmg, crit) = roll(&mut rng, stats.crit, area.damage);
                hits.write(Hit {
                    owner: area.owner,
                    target: f.e,
                    damage: dmg,
                    dot: area.dot,
                    dot_dur: area.dot_dur,
                    effect: area.effect,
                    mark: area.mark,
                    mark_dur: 1.5,
                    splash: 0.0,
                    crit,
                    color: COLOR_SPELL,
                    proc: false,
                    spell: area.spell,
                });
            }
        }
        if area.left <= 0.0 {
            commands.entity(e).despawn();
        }
    }

    for (e, tf, mut totem) in &mut totems {
        totem.left -= dt;
        if totem.left <= 0.0 {
            commands
                .entity(e)
                .remove::<Totem>()
                .insert(benilla_world::model_fade::DespawnFade::default());
            continue;
        }
        totem.tick -= dt;
        if totem.tick > 0.0 {
            continue;
        }
        let pos = tf.translation;
        let stats = stats_of(totem.owner);
        if totem.pulse {
            let targets = around(&live, pos, totem.radius);
            totem.tick = 2.0;
            if targets.is_empty() {
                continue;
            }
            fx.cast(e, totem.cast, &[], None);
            for f in &targets {
                let (dmg, crit) = roll(&mut rng, stats.crit, totem.damage);
                hits.write(Hit {
                    color: COLOR_SPELL,
                    spell: totem.cast,
                    ..Hit::direct(totem.owner, f.e, dmg, crit)
                });
            }
        } else {
            totem.tick = 1.0;
            let target = live
                .iter()
                .map(|f| (flat(f.pos - pos).length(), f))
                .filter(|(d, _)| *d <= totem.range)
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let Some((dist, f)) = target else { continue };
            fx.cast(e, totem.cast, &[f.e], None);
            let speed = spells
                .as_deref()
                .and_then(|s| s.catalog.get(totem.cast))
                .map_or(0.0, |d| d.speed);
            let delay = if speed > 0.0 { dist / speed } else { 0.0 };
            let (dmg, crit) = roll(&mut rng, stats.crit, totem.damage);
            schedule(
                &mut pending,
                &mut hits,
                now,
                delay,
                Hit {
                    color: COLOR_SPELL,
                    spell: totem.cast,
                    ..Hit::direct(totem.owner, f.e, dmg, crit)
                },
            );
        }
    }
}

/// Land every hit due: damage and its number, the over-time part, crowd control, the marks, and
/// death with its loot. Each blow's hero is credited with its damage, drain and kill.
#[allow(clippy::type_complexity)]
pub(super) fn apply_hits(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    mut pending: ResMut<PendingHits>,
    mut incoming: MessageReader<Hit>,
    mut enemies: Query<(
        Entity,
        &Guid,
        &Transform,
        &mut Enemy,
        &mut ObjectStore,
        &mut Marks,
    )>,
    mut heroes: Query<&mut HeroState>,
    mut fx: Fx,
    mut loot: MessageWriter<Loot>,
    mut fields: MessageWriter<FieldChanged>,
    mut rng: ResMut<Rng>,
) {
    let dt = time.delta_secs();
    let now = run.elapsed;
    // The run items' arcs and bursts this frame, aimed once every hit has landed.
    let mut procs: Vec<ItemProc> = Vec::new();
    let mut due: Vec<Hit> = incoming.read().cloned().collect();
    pending.0.retain(|(t, h)| {
        if *t <= now {
            due.push(h.clone());
            false
        } else {
            true
        }
    });

    // Splash reaches the target's neighbours at seventy percent.
    let mut all = Vec::with_capacity(due.len());
    for h in due {
        if h.splash > 0.0 {
            if let Ok((_, _, center, ..)) = enemies.get(h.target) {
                let c = center.translation;
                for (e, _, tf, en, ..) in enemies.iter() {
                    if e != h.target && !en.dead && flat(tf.translation - c).length() <= h.splash {
                        all.push(Hit {
                            target: e,
                            damage: h.damage * 0.7,
                            splash: 0.0,
                            mark: 0,
                            ..h.clone()
                        });
                    }
                }
            }
        }
        all.push(Hit { splash: 0.0, ..h });
    }

    let mut kill = |commands: &mut Commands,
                    run: &mut Run,
                    owner: Option<&mut HeroState>,
                    e: Entity,
                    pos: Vec3,
                    en: &mut Enemy| {
        en.dead = true;
        en.hp = 0.0;
        en.dots.clear();
        enemies::mark_dying(commands, e);
        loot.write(Loot {
            pos,
            xp: en.xp,
            boss: en.boss,
            boss_slot: en.boss_slot,
            elite: en.elite,
        });
        if let Some(owner) = owner {
            owner.kills += 1;
        }
        if en.boss {
            run.banner = Some((format!("{} is slain!", en.def.name), 3.0));
        }
        if en.final_boss {
            run.final_slain = true;
        }
    };

    for h in all {
        let Ok((e, guid, tf, mut en, mut store, mut marks)) = enemies.get_mut(h.target) else {
            continue;
        };
        if en.dead {
            continue;
        }
        let mut owner = heroes.get_mut(h.owner).ok();
        // A proc's own hit fires nothing more.
        let effects: Vec<ItemEffect> = match (&owner, h.proc) {
            (Some(o), false) => o.item_effects().collect(),
            _ => Vec::new(),
        };
        let mut dmg = h.damage;
        if let Effect::Execute(threshold) = h.effect {
            if !en.boss && en.hp / en.max_hp <= threshold {
                dmg = dmg.max(en.hp);
            }
        }
        // A run item's execute finishes what the hit leaves under its share.
        if !en.boss && dmg > 0.0 && (en.hp - dmg) / en.max_hp <= execute_share(&effects) {
            dmg = dmg.max(en.hp);
        }
        if dmg > 0.0 {
            en.hp -= dmg;
            units::set_health(&mut store, en.hp / en.max_hp);
            let shove = data::knock_impulse(dmg, en.max_hp, en.boss, en.elite.is_some());
            en.knock = (en.knock + shove).min(data::KNOCK_MAX);
            fx.number(e, dmg, h.crit, h.color);
            if let Some(o) = owner.as_deref_mut() {
                o.damage_done += f64::from(dmg);
                credit(&mut o.v, h.spell, dmg);
                if let Effect::Drain(frac) = h.effect {
                    if o.alive() {
                        let max = o.stats().max_hp;
                        o.hp = (o.hp + dmg * frac).min(max);
                    }
                }
                on_hit(
                    o,
                    h.owner,
                    &effects,
                    dmg,
                    h.target,
                    tf.translation,
                    &mut rng,
                    &mut procs,
                );
                if en.hp > 0.0 {
                    let might = o.stats().might;
                    if let Some(spell) = afflict(&mut en, h.owner, &effects, might, &mut rng) {
                        fx.impact(e, spell);
                    }
                }
            }
        }
        if h.dot > 0.0 && h.dot_dur > 0.0 {
            if en.dots.len() >= 6 {
                en.dots.remove(0);
            }
            en.dots.push(enemies::Dot {
                owner: h.owner,
                spell: h.spell,
                dps: h.dot,
                left: h.dot_dur,
                tick: 1.0,
            });
        }
        let boss = en.boss;
        match h.effect {
            Effect::Slow(m, t) => {
                let m = if boss { m.max(0.75) } else { m };
                en.slow = if en.slow_t > 0.0 { en.slow.min(m) } else { m };
                en.slow_t = en.slow_t.max(t);
            }
            Effect::Root(t) => en.root_t = en.root_t.max(if boss { t * 0.3 } else { t }),
            Effect::Stun(t) => en.stun_t = en.stun_t.max(if boss { t * 0.3 } else { t }),
            Effect::Fear(t) if !boss => en.fear_t = en.fear_t.max(t),
            Effect::Weaken(m, t) => {
                en.weaken = m;
                en.weaken_t = t;
            }
            _ => {}
        }
        if h.mark != 0 {
            marks.add(h.mark, h.mark_dur);
        }
        if en.hp <= 0.0 {
            kill(
                &mut commands,
                &mut run,
                owner.as_deref_mut(),
                e,
                tf.translation,
                &mut en,
            );
            if let Some(o) = owner.as_deref_mut() {
                on_kill(o, h.owner, &effects, tf.translation, &mut rng, &mut procs);
            }
            units::kill_unit(&mut store);
            marks.clear();
        }
        marks.flush(e, guid.0, &mut store, &mut fields);
    }

    // The over-time ticks, once a second each, credited to whoever laid them.
    for (e, guid, tf, mut en, mut store, mut marks) in &mut enemies {
        if en.dead || en.dots.is_empty() {
            continue;
        }
        let mut dealt: Vec<(Entity, f32, u32)> = Vec::new();
        for d in &mut en.dots {
            d.left -= dt;
            d.tick -= dt;
            if d.tick <= 0.0 {
                d.tick += 1.0;
                dealt.push((d.owner, d.dps, d.spell));
            }
        }
        en.dots.retain(|d| d.left > 0.0);
        let total: f32 = dealt.iter().map(|d| d.1).sum();
        if total > 0.0 {
            en.hp -= total;
            units::set_health(&mut store, en.hp / en.max_hp);
            for &(owner, dps, spell) in &dealt {
                if let Ok(mut o) = heroes.get_mut(owner) {
                    o.damage_done += f64::from(dps);
                    credit(&mut o.v, spell, dps);
                }
            }
            fx.number(e, total, false, COLOR_SPELL);
            if en.hp <= 0.0 {
                let killer = dealt.last().map(|d| d.0);
                let mut owner = killer.and_then(|k| heroes.get_mut(k).ok());
                kill(
                    &mut commands,
                    &mut run,
                    owner.as_deref_mut(),
                    e,
                    tf.translation,
                    &mut en,
                );
                if let (Some(o), Some(k)) = (owner.as_deref_mut(), killer) {
                    let effects: Vec<ItemEffect> = o.item_effects().collect();
                    on_kill(o, k, &effects, tf.translation, &mut rng, &mut procs);
                }
                units::kill_unit(&mut store);
                marks.clear();
                marks.flush(e, guid.0, &mut store, &mut fields);
            }
        }
    }

    // The arcs and bursts: their foes picked now every hit has landed, struck a beat later.
    if procs.is_empty() {
        return;
    }
    let live: Vec<(Entity, Vec3)> = enemies
        .iter()
        .filter(|(.., en, _, _)| !en.dead)
        .map(|(e, _, tf, ..)| (e, tf.translation))
        .collect();
    for p in procs {
        let aimed = aim_proc(&p, &live);
        for &(delay, target) in &aimed {
            pending.0.push((
                now + delay,
                Hit {
                    color: COLOR_SPELL,
                    proc: true,
                    spell: p.spell,
                    ..Hit::direct(p.owner, target, p.damage, false)
                },
            ));
        }
        let struck: Vec<Entity> = aimed.into_iter().map(|(_, e)| e).collect();
        if struck.is_empty() {
            continue;
        }
        match p.kind {
            ProcKind::Arc { .. } => fx.cast(p.owner, p.spell, &struck, None),
            ProcKind::Burst { .. } => {
                for &t in struck.iter().take(12) {
                    fx.impact(t, p.spell);
                }
            }
        }
    }
}

/// Count `dmg` to `spell` on the hero's damage meter.
pub(super) fn credit(v: &mut Vitals, spell: u32, dmg: f32) {
    match v.damage_by.iter_mut().find(|(s, _)| *s == spell) {
        Some((_, d)) => *d += f64::from(dmg),
        None => v.damage_by.push((spell, f64::from(dmg))),
    }
}

/// A run item's arc or burst, set off by a hit or a kill, to aim once the frame's hits land.
#[derive(Clone, Debug, PartialEq)]
struct ItemProc {
    owner: Entity,
    at: Vec3,
    damage: f32,
    spell: u32,
    kind: ProcKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ProcKind {
    /// From the foe struck (`from`, not struck again) to up to `jumps` more, each the nearest to
    /// the last within [`data::ITEM_CHAIN_REACH`].
    Arc { from: Entity, jumps: u8 },
    /// Everything within `radius` of where the foe fell.
    Burst { radius: f32 },
}

/// What a hit sets off of the striker's items (`owner` is the hero's entity): the lifesteal at
/// once, an arc on its chance.
#[allow(clippy::too_many_arguments)]
fn on_hit(
    o: &mut HeroState,
    owner: Entity,
    effects: &[ItemEffect],
    dmg: f32,
    target: Entity,
    at: Vec3,
    rng: &mut Rng,
    procs: &mut Vec<ItemProc>,
) {
    for e in effects {
        match *e {
            ItemEffect::Lifesteal(share) if o.alive() => {
                let max = o.stats().max_hp;
                o.hp = (o.hp + dmg * share).min(max);
            }
            ItemEffect::Chain {
                chance,
                damage,
                jumps,
                spell,
            } if rng.f32() < chance => procs.push(ItemProc {
                owner,
                at,
                damage: damage * o.stats().might,
                spell,
                kind: ProcKind::Arc {
                    from: target,
                    jumps,
                },
            }),
            _ => {}
        }
    }
}

/// The share of its health under which the hero's items finish a foe; 0 for none.
fn execute_share(effects: &[ItemEffect]) -> f32 {
    effects
        .iter()
        .map(|e| match *e {
            ItemEffect::Execute(t) => t,
            _ => 0.0,
        })
        .fold(0.0, f32::max)
}

/// What a hit lays on its foe of the striker's items: a burn and a chill, each on its chance.
/// Returns the burn's spell, for its visual, when one caught.
fn afflict(
    en: &mut Enemy,
    owner: Entity,
    effects: &[ItemEffect],
    might: f32,
    rng: &mut Rng,
) -> Option<u32> {
    let mut lit = None;
    for e in effects {
        match *e {
            ItemEffect::Ignite {
                chance,
                dps,
                secs,
                spell,
            } if rng.f32() < chance => {
                // One burn per item: a new one starts it over.
                en.dots.retain(|d| !(d.owner == owner && d.spell == spell));
                if en.dots.len() >= 6 {
                    en.dots.remove(0);
                }
                en.dots.push(enemies::Dot {
                    owner,
                    spell,
                    dps: dps * might,
                    left: secs,
                    tick: 1.0,
                });
                lit = Some(spell);
            }
            ItemEffect::Chill { chance, slow, secs } if rng.f32() < chance => {
                en.slow = en.slow.min(slow);
                en.slow_t = en.slow_t.max(if en.boss { secs * 0.3 } else { secs });
            }
            _ => {}
        }
    }
    lit
}

/// Every hero's novas: each `Nova` item strikes everything near the hero on its own clock.
pub(super) fn item_novas(
    time: Res<Time>,
    mut heroes: Query<(Entity, &Transform, &mut HeroState), With<Hero>>,
    enemies: Query<(Entity, &Transform, &Enemy)>,
    mut hits: MessageWriter<Hit>,
    mut fx: Fx,
) {
    let dt = time.delta_secs();
    for (he, tf, mut h) in &mut heroes {
        let nova = h.item_effects().find_map(|e| match e {
            ItemEffect::Nova {
                every,
                damage,
                radius,
                spell,
            } => Some((every, damage, radius, spell)),
            _ => None,
        });
        let Some((every, damage, radius, spell)) = nova else {
            continue;
        };
        if !h.alive() {
            continue;
        }
        h.nova_t -= dt;
        if h.nova_t > 0.0 {
            continue;
        }
        h.nova_t = every.max(0.5);
        let at = tf.translation;
        let mut struck: Vec<(Entity, f32)> = enemies
            .iter()
            .filter(|(.., en)| !en.dead)
            .map(|(e, t, _)| (e, flat(t.translation - at).length()))
            .filter(|(_, d)| *d <= radius)
            .collect();
        struck.sort_by(|a, b| a.1.total_cmp(&b.1));
        struck.truncate(data::MAX_NOVA_HITS);
        if struck.is_empty() {
            continue;
        }
        let might = h.stats().might;
        for &(e, _) in &struck {
            hits.write(Hit {
                color: COLOR_SPELL,
                proc: true,
                spell,
                ..Hit::direct(he, e, damage * might, false)
            });
        }
        let shown: Vec<Entity> = struck.iter().map(|(e, _)| *e).collect();
        fx.cast(he, spell, &shown, None);
    }
}

/// What a kill sets off of the killer's items: the heal at once, a burst on its chance.
fn on_kill(
    o: &mut HeroState,
    owner: Entity,
    effects: &[ItemEffect],
    at: Vec3,
    rng: &mut Rng,
    procs: &mut Vec<ItemProc>,
) {
    for e in effects {
        match *e {
            ItemEffect::KillHeal(hp) if o.alive() => {
                let max = o.stats().max_hp;
                o.hp = (o.hp + hp).min(max);
            }
            ItemEffect::KillShield { amount, max } if o.alive() => {
                o.shield = (o.shield + amount).min(max.max(o.shield));
                o.shield_t = o.shield_t.max(data::KILL_SHIELD_SECS);
            }
            ItemEffect::Bounty { chance, gold } if rng.f32() < chance => {
                let gain = gold * o.stats().gold;
                o.gold += gain;
            }
            ItemEffect::Burst {
                chance,
                damage,
                radius,
                spell,
            } if rng.f32() < chance => procs.push(ItemProc {
                owner,
                at,
                damage: damage * o.stats().might,
                spell,
                kind: ProcKind::Burst { radius },
            }),
            _ => {}
        }
    }
}

/// Whom an arc or a burst strikes among the living foes, and how long after the frame each.
fn aim_proc(p: &ItemProc, live: &[(Entity, Vec3)]) -> Vec<(f32, Entity)> {
    match p.kind {
        ProcKind::Arc { from, jumps } => {
            let mut out: Vec<(f32, Entity)> = Vec::new();
            let mut last = p.at;
            for k in 0..jumps {
                let next = live
                    .iter()
                    .filter(|(e, _)| *e != from && !out.iter().any(|(_, o)| o == e))
                    .map(|&(e, pos)| (e, pos, flat(pos - last).length()))
                    .filter(|(.., d)| *d <= data::ITEM_CHAIN_REACH)
                    .min_by(|a, b| a.2.total_cmp(&b.2));
                let Some((e, pos, _)) = next else { break };
                out.push((0.08 * f32::from(k + 1), e));
                last = pos;
            }
            out
        }
        ProcKind::Burst { radius } => live
            .iter()
            .filter(|(_, pos)| flat(*pos - p.at).length() <= radius)
            .map(|&(e, _)| (0.05, e))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn foe(n: u32, dist: f32) -> Foe {
        Foe {
            e: Entity::from_raw_u32(n).unwrap(),
            pos: Vec3::ZERO,
            hp: 10.0,
            dist,
            radius: 0.5,
        }
    }

    fn hero_with(item: &str, stacks: u8) -> HeroState {
        let mut v = Vitals::new(&data::CLASSES[0], 1);
        v.run_items.push((data::run_item(item).unwrap(), stacks));
        HeroState { slot: 0, v }
    }

    fn e(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn an_arc_leaps_foe_to_foe_within_reach_and_skips_the_first() {
        let reach = data::ITEM_CHAIN_REACH;
        let live = [
            (e(1), Vec3::ZERO),
            (e(2), Vec3::X * (reach - 1.0)),
            (e(3), Vec3::X * (2.0 * reach - 2.0)),
            (e(4), Vec3::X * 100.0),
        ];
        let arc = ItemProc {
            owner: e(9),
            at: Vec3::ZERO,
            damage: 10.0,
            spell: 421,
            kind: ProcKind::Arc {
                from: e(1),
                jumps: 4,
            },
        };
        let hit: Vec<Entity> = aim_proc(&arc, &live).into_iter().map(|(_, t)| t).collect();
        assert_eq!(hit, [e(2), e(3)], "the far one is out of reach");
        let burst = ItemProc {
            kind: ProcKind::Burst { radius: 5.0 },
            ..arc
        };
        let hit: Vec<Entity> = aim_proc(&burst, &live)
            .into_iter()
            .map(|(_, t)| t)
            .collect();
        assert_eq!(hit, [e(1)]);
    }

    #[test]
    fn items_fire_on_hits_and_kills() {
        let mut rng = Rng::seeded(1);
        // Lifesteal heals off the damage dealt.
        let mut h = hero_with("vampiric_fang", 4);
        h.hp = 10.0;
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        let mut procs = Vec::new();
        on_hit(
            &mut h,
            e(9),
            &fx,
            100.0,
            e(1),
            Vec3::ZERO,
            &mut rng,
            &mut procs,
        );
        assert!((h.hp - 16.0).abs() < 1e-3, "{}", h.hp);
        assert!(procs.is_empty());
        // A kill heals with the soul shard.
        let mut h = hero_with("soulfeast", 2);
        h.hp = 10.0;
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        on_kill(&mut h, e(9), &fx, Vec3::ZERO, &mut rng, &mut procs);
        assert!((h.hp - 13.0).abs() < 1e-3);
        // The arc and the burst fire about as often as their chance says.
        let mut h = hero_with("storm_charm", 5);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        for _ in 0..1000 {
            on_hit(
                &mut h,
                e(9),
                &fx,
                1.0,
                e(1),
                Vec3::ZERO,
                &mut rng,
                &mut procs,
            );
        }
        let arcs = procs.len();
        assert!((220..=380).contains(&arcs), "{arcs} arcs at 30%");
        assert!(procs.iter().all(|p| p.owner == e(9)));
        procs.clear();
        let mut h = hero_with("volatile_ember", 1);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        for _ in 0..1000 {
            on_kill(&mut h, e(9), &fx, Vec3::ZERO, &mut rng, &mut procs);
        }
        assert!(
            (100..=200).contains(&procs.len()),
            "{} bursts at 15%",
            procs.len()
        );
    }

    fn enemy(boss: bool) -> Enemy {
        let def = &data::roster(0).enemies[0];
        Enemy {
            def,
            hp: 100.0,
            max_hp: 100.0,
            speed: 5.0,
            damage: 5.0,
            radius: 0.5,
            xp: 1,
            boss,
            boss_slot: None,
            final_boss: false,
            elite: None,
            home: None,
            attack_cd: 0.0,
            slow: 1.0,
            slow_t: 0.0,
            root_t: 0.0,
            stun_t: 0.0,
            fear_t: 0.0,
            weaken: 1.0,
            weaken_t: 0.0,
            dots: Vec::new(),
            dead: false,
        }
    }

    #[test]
    fn the_new_items_burn_chill_finish_shield_and_pay() {
        let mut rng = Rng::seeded(7);
        // A burn and a chill catch about as often as their chance, one burn per item at once.
        let h = hero_with("smoldering_brand", 5);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        let mut burns = 0;
        for _ in 0..1000 {
            let mut en = enemy(false);
            if afflict(&mut en, e(9), &fx, 1.0, &mut rng).is_some() {
                burns += 1;
                assert_eq!(en.dots.len(), 1);
                assert_eq!((en.dots[0].dps, en.dots[0].spell), (25.0, 348));
            }
        }
        assert!((320..=480).contains(&burns), "{burns} burns at 40%");
        let mut en = enemy(false);
        for _ in 0..50 {
            afflict(&mut en, e(9), &fx, 1.0, &mut rng);
        }
        assert_eq!(en.dots.len(), 1, "a new burn starts the old one over");
        let h = hero_with("frostbite_locket", 9);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        let mut en = enemy(false);
        for _ in 0..50 {
            afflict(&mut en, e(9), &fx, 1.0, &mut rng);
        }
        assert_eq!((en.slow, en.slow_t), (0.5, 2.0));
        // An execute finishes under its share, capped.
        let h = hero_with("headsmans_axe", 2);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        assert!((execute_share(&fx) - 0.12).abs() < 1e-6);
        let h = hero_with("headsmans_axe", data::MAX_RUN_ITEM_STACKS);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        assert_eq!(execute_share(&fx), data::MAX_EXECUTE);
        assert_eq!(execute_share(&[]), 0.0);
        // A kill's shield grows up to its most; a bounty pays.
        let mut h = hero_with("barrier_sigil", 1);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        let mut procs = Vec::new();
        for _ in 0..100 {
            on_kill(&mut h, e(9), &fx, Vec3::ZERO, &mut rng, &mut procs);
        }
        assert_eq!(h.shield, 25.0);
        assert_eq!(h.shield_t, data::KILL_SHIELD_SECS);
        let mut h = hero_with("goblin_purse", 5);
        let fx: Vec<ItemEffect> = h.item_effects().collect();
        let before = h.gold;
        for _ in 0..1000 {
            on_kill(&mut h, e(9), &fx, Vec3::ZERO, &mut rng, &mut procs);
        }
        assert!(h.gold > before + 100.0, "{}", h.gold);
        // Dodges add up, to a cap.
        let mut h = hero_with("evasion_charm", 3);
        assert!((enemies::dodge_chance(&h) - 0.12).abs() < 1e-6);
        h.run_items[0].1 = data::MAX_RUN_ITEM_STACKS;
        assert_eq!(enemies::dodge_chance(&h), data::MAX_DODGE);
    }

    #[test]
    fn the_target_is_struck_first_while_in_reach() {
        let foes = [foe(1, 2.0), foe(2, 5.0), foe(3, 30.0)];
        let ids = |v: &[Foe]| v.iter().map(|f| f.e).collect::<Vec<_>>();
        // No target: nearest first.
        assert_eq!(ids(&aimed_order(&foes, None)), ids(&foes));
        // The target in reach is taken before a nearer one.
        let aimed = aimed_order(&foes, Some(foes[1].e));
        assert_eq!(nearest(&aimed, 10.0, 1)[0].e, foes[1].e);
        // Out of reach, the nearest is taken instead.
        let aimed = aimed_order(&foes, Some(foes[2].e));
        assert_eq!(nearest(&aimed, 10.0, 1)[0].e, foes[0].e);
    }
}
