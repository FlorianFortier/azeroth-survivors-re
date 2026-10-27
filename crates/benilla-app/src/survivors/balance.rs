//! The balance simulator: whole runs played headless, with no game data, no window and no GPU,
//! through the mode's own simulation (`SimSet`'s systems, in their order) on an empty, flat world.
//!
//! A simple pilot plays the hero the way a player new to the run would: it circles the field's
//! middle, kiting the horde; walks to a chest once it has the gold; walks into a portal once one is
//! open; takes the rarest card it is dealt and the first blessing it is offered. What it measures
//! for every stage — how long it took, the level reached, the kills, the damage taken and the
//! lowest health — is what the numbers in `data.rs` are tuned against
//! ([`BANDS`], held by the tests).
//!
//! `cargo test -p benilla-app --lib --no-default-features survivors::balance -- --ignored
//! --nocapture` runs every class through a whole run and prints the table; the suite itself runs
//! a short one, and the first stage of a few classes against the bands.

#![cfg(test)]

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use survivors_net::game::CardAction;

use super::data::{self, CardRarity};
use super::hero::{self, Hero};
use super::treasure::Chest;
use super::{
    abilities, enemies, legs, market, mobility, offer, outfit, treasure, HeroState, LocalHero, Rng,
    Run, Selection, Session, Vitals,
};

/// The simulation's step: coarser than a frame, fine for what is measured.
const DT: f32 = 0.1;
/// How far from the field's middle the pilot circles.
const KITE_RADIUS: f32 = 18.0;
/// Foes the pilot minds, and how far ahead it looks.
const LOOK_REACH: f32 = 20.0;
const LOOK_SECS: f32 = 1.5;

/// One stage of a run, as the pilot played it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct StageLog {
    /// Seconds the stage took, boss and all.
    pub secs: f32,
    /// The hero's level when it ended.
    pub level: u32,
    pub kills: u32,
    /// Health lost over the stage, before heals.
    pub taken: f32,
    /// The lowest health share the hero fell to.
    pub low: f32,
    /// The stage's final boss fell.
    pub cleared: bool,
    /// When the boss portal opened and when the boss was called, on the stage's clock.
    pub portal: Option<f32>,
    pub called: Option<f32>,
    /// The final boss's health share at the end, while it stood.
    pub boss_left: Option<f32>,
}

/// A whole run: its stages, and how it ended.
#[derive(Clone, Debug, Default)]
pub(super) struct RunLog {
    pub class: &'static str,
    pub stages: Vec<StageLog>,
    pub won: bool,
    pub lost: bool,
    /// Seconds of play in all.
    pub secs: f32,
    /// The spells that did the most damage, most first.
    pub top: Vec<(String, f64)>,
    /// Every [`TRACE_EVERY`] seconds, what the hero faced.
    pub trace: Vec<TracePoint>,
}

const TRACE_EVERY: f32 = 10.0;

/// The run at one moment of the trace.
#[derive(Clone, Debug, Default)]
pub(super) struct TracePoint {
    pub clock: f32,
    pub hp: f32,
    pub alive: usize,
    /// The most foes within 4 yd of the hero at once since the last point.
    pub near: usize,
    /// Health lost since the last point, and how much of it in reach of a boss.
    pub taken: f32,
    pub by_boss: f32,
    pub surge: bool,
}

/// A headless world that plays `SimSet` and nothing else.
fn sim_app(class: usize, zone: usize, seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(DT)))
        .insert_resource(Rng::seeded(seed))
        .insert_resource(Selection {
            map: zone,
            stage: zone,
            ..Selection::default()
        })
        .init_resource::<Session>()
        .init_resource::<Run>()
        .init_resource::<super::targeting::HeroTarget>()
        .init_resource::<abilities::PendingHits>()
        .init_resource::<enemies::Spawner>()
        .init_resource::<enemies::Vacuum>()
        .init_resource::<treasure::Field>()
        .init_resource::<market::Market>()
        .init_resource::<market::BuyQueue>()
        .init_resource::<legs::Legs>()
        .init_resource::<mobility::MobilityQueue>()
        .init_resource::<crate::creature_anim::PlaySeq>()
        .add_message::<abilities::Hit>()
        .add_message::<enemies::Loot>()
        .add_message::<super::feel::Struck>()
        .add_message::<crate::creature_anim::CastEvent>()
        .add_message::<crate::creature_anim::SpellGoTargets>()
        .add_message::<crate::creature_anim::SwingMessage>()
        .add_message::<crate::combat_text::CombatTextSpawn>()
        .add_message::<crate::net::FieldChanged>()
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
                .chain(),
        );
    benilla_world::world_point::init_world_point_resources(app.world_mut());

    // The hero, at the field's middle, with nothing from a profile: what the run alone gives.
    let class = &data::CLASSES[class.min(data::CLASSES.len() - 1)];
    let center = hero::arena(&data::MAPS[zone.min(data::MAPS.len() - 1)]);
    let world = app.world_mut();
    {
        let mut commands = world.commands();
        let e = hero::spawn_hero(
            &mut commands,
            class,
            &outfit::Body::Trainer { display: 1 },
            1,
            0,
            1,
            center,
        );
        commands.entity(e).insert((
            LocalHero,
            HeroState {
                slot: 0,
                v: Vitals::new(class, 1).with_meta(Vec::new(), &[]),
            },
        ));
    }
    world.flush();
    let mut run = world.resource_mut::<Run>();
    run.me = Vitals::new(class, 1);
    run.next_guid = 100;
    run.begun = true;
    app
}

/// The pilot's step: where the hero goes, and its moves at the cards and the shrines.
fn pilot(app: &mut App, t: f32) {
    let world = app.world_mut();
    let center = {
        let sel = world.resource::<Selection>();
        hero::arena(sel.stage_def())
    };
    let gates: Vec<Vec3> = world
        .resource::<Run>()
        .gates
        .iter()
        .filter(|g| g.open)
        .map(|g| g.pos)
        .collect();
    let chest_cost = world.resource::<Run>().chest_cost;
    let foes: Vec<Vec3> = world
        .query::<(&Transform, &enemies::Enemy)>()
        .iter(world)
        .filter(|(_, e)| !e.dead)
        .map(|(t, _)| t.translation)
        .collect();
    let chests: Vec<Vec3> = world
        .query_filtered::<&Transform, With<Chest>>()
        .iter(world)
        .map(|t| t.translation)
        .collect();

    // The cards and the blessings, before anything moves.
    let mut rng = Rng::seeded(t.to_bits() as u64);
    let mut heroes = world.query_filtered::<(&mut Transform, &mut HeroState), With<Hero>>();
    for (mut tf, mut h) in heroes.iter_mut(world) {
        if !h.alive() {
            continue;
        }
        let v = &mut h.v;
        if v.pending_levels > 0 && v.offer.is_empty() {
            v.offer = offer::deal(v, &mut rng, data::CARDS_DEALT, &[]);
        }
        if let Some(best) = v
            .offer
            .iter()
            .max_by_key(|d| CardRarity::ALL.iter().position(|r| *r == d.rarity))
            .cloned()
        {
            let wire = best.to_wire();
            if !offer::apply_action(v, &CardAction::Pick(wire), &mut rng) {
                offer::apply_action(v, &CardAction::Skip, &mut rng);
            }
        }
        if let Some(b) = v.shrine_offer.first().copied() {
            treasure::take_boon(v, data::bonus_to_wire(&b.bonus));
        }

        // Where to: an open portal, a chest it can pay for, else round the middle.
        let at = tf.translation;
        let near = |list: &[Vec3]| {
            list.iter()
                .copied()
                .min_by(|a, b| (*a - at).length().total_cmp(&(*b - at).length()))
        };
        let gate = near(&gates);
        let goal = gate.or_else(|| {
            (v.gold >= chest_cost as f32 || v.keys > 0)
                .then(|| near(&chests))
                .flatten()
        });
        let speed = v.stats().speed;
        let engage = (data::ability(v.class.start).range * 0.7).max(2.5);
        // A fighter in melee brawls; a caster keeps its distance.
        let (caution, pull) = if engage < 6.0 { (25.0, 1.5) } else { (40.0, 0.3) };
        // A player looks ahead: of sixteen headings, the one whose point a second and a half on
        // is clearest of foes, kept near the field's middle, and toward the goal if there is one.
        let near: Vec<Vec3> = foes
            .iter()
            .copied()
            .filter(|f| super::units::flat(*f - at).length() < LOOK_REACH)
            .collect();
        // A fighter in melee takes on a couple at once: the two nearest do not count against it.
        let melee = engage < 6.0;
        let danger = |p: Vec3| -> f32 {
            let mut d: Vec<f32> = near
                .iter()
                .map(|f| super::units::flat(p - *f).length().max(0.5))
                .collect();
            if melee {
                d.sort_by(f32::total_cmp);
                d.drain(..d.len().min(2));
            }
            d.iter().map(|d| 1.0 / (d * d)).sum()
        };
        // In a portal, standing is the point: the crowd matters less than the charge.
        let caution = if gate.is_some() { caution * 0.5 } else { caution };
        let mut best = (f32::INFINITY, Vec3::ZERO);
        for k in 0..=16 {
            // Sixteen headings, and standing still.
            let a = k as f32 / 16.0 * std::f32::consts::TAU;
            let dir = if k == 16 {
                Vec3::ZERO
            } else {
                Vec3::new(a.cos(), 0.0, a.sin())
            };
            let ahead = at + dir * speed * LOOK_SECS;
            let mut score = (danger(ahead) + danger(at + dir * speed * 0.5)) * caution;
            let off = super::units::flat(ahead - center).length();
            score += ((off - KITE_RADIUS).max(0.0) / 10.0).powi(2);
            match (gate, goal) {
                // A portal wants the hero inside it while it charges, moving or not.
                (Some(g), _) => {
                    let out = (super::units::flat(at + dir * speed * 0.5 - g).length()
                        - data::PORTAL_RADIUS * 0.6)
                        .max(0.0);
                    score += out * out * 4.0;
                }
                (None, Some(g)) => score += super::units::flat(ahead - g).length() * 0.05,
                // With nothing to go to, it keeps its spells' reach of the horde.
                (None, None) => {
                    let d = near
                        .iter()
                        .map(|f| super::units::flat(ahead - *f).length())
                        .fold(f32::INFINITY, f32::min);
                    if d.is_finite() {
                        score += (d - engage).max(0.0) * pull;
                    }
                }
            }
            if score < best.0 {
                best = (score, dir);
            }
        }
        // At the goal and in no danger, it stands (a chest wants the hero still).
        let at_goal = goal.is_some_and(|g| super::units::flat(g - at).length() < 0.8);
        if !(at_goal && danger(at) < 0.05) {
            tf.translation += best.1 * speed * DT;
        }
    }
}

/// Play a run of `class` on `zone` until it is won, lost, or `limit` seconds have gone.
pub(super) fn play(class: usize, zone: usize, seed: u64, limit: f32) -> RunLog {
    let mut app = sim_app(class, zone, seed);
    let mut log = RunLog {
        class: data::CLASSES[class.min(data::CLASSES.len() - 1)].name,
        ..RunLog::default()
    };
    let mut stage = StageLog {
        low: 1.0,
        ..StageLog::default()
    };
    let mut leg = 0u8;
    let mut last_hp: Option<f32> = None;
    let mut window = TracePoint::default();
    let mut t = 0.0;
    // A first update seats the clock.
    app.update();
    while t < limit {
        pilot(&mut app, t);
        app.update();
        t += DT;
        let world = app.world_mut();
        let (hp, max, level, kills) = {
            let mut q = world.query::<&HeroState>();
            let h = q.iter(world).next().expect("the hero");
            (h.hp, h.stats().max_hp, h.level, h.kills)
        };
        let at = {
            let mut q = world.query_filtered::<&Transform, With<Hero>>();
            q.iter(world).next().map_or(Vec3::ZERO, |t| t.translation)
        };
        let (near, boss_near) = {
            let mut q = world.query::<(&Transform, &enemies::Enemy)>();
            let mut n = 0;
            let mut b = false;
            for (t, e) in q.iter(world) {
                if e.dead {
                    continue;
                }
                let d = super::units::flat(t.translation - at).length();
                if d < 4.0 {
                    n += 1;
                }
                if e.boss && d < 6.0 {
                    b = true;
                }
            }
            (n, b)
        };
        window.near = window.near.max(near);
        if let Some(prev) = last_hp {
            if hp < prev {
                stage.taken += prev - hp;
                window.taken += prev - hp;
                if boss_near {
                    window.by_boss += prev - hp;
                }
            }
        }
        last_hp = Some(hp);
        stage.low = stage.low.min((hp / max.max(1.0)).max(0.0));
        stage.level = level;
        stage.kills = kills;
        stage.secs += DT;
        if (t / TRACE_EVERY).floor() != ((t - DT) / TRACE_EVERY).floor() || hp <= 0.0 {
            let mut q = world.query::<&enemies::Enemy>();
            let alive = q.iter(world).filter(|e| !e.dead).count();
            let clock = world.resource::<Run>().elapsed;
            log.trace.push(TracePoint {
                clock,
                hp,
                alive,
                surge: data::surging(clock),
                ..std::mem::take(&mut window)
            });
        }
        let boss_left = {
            let mut q = world.query::<&enemies::Enemy>();
            q.iter(world)
                .find(|e| e.final_boss && !e.dead)
                .map(|e| e.hp / e.max_hp)
        };
        let run = world.resource::<Run>();
        if run.leg_done {
            stage.cleared = true;
        }
        if stage.portal.is_none() && run.gates.iter().any(|g| g.open) {
            stage.portal = Some(run.elapsed);
        }
        if stage.called.is_none() && boss_left.is_some() {
            stage.called = Some(run.elapsed);
        }
        if boss_left.is_some() {
            stage.boss_left = boss_left;
        }
        let (won, lost, now) = (run.won, run.lost, run.leg);
        if now != leg {
            leg = now;
            log.stages.push(std::mem::replace(
                &mut stage,
                StageLog {
                    low: 1.0,
                    level,
                    kills,
                    ..StageLog::default()
                },
            ));
        }
        if won || lost {
            log.won = won;
            log.lost = lost;
            break;
        }
    }
    log.stages.push(stage);
    log.secs = t;
    let world = app.world_mut();
    let mut q = world.query::<&HeroState>();
    if let Some(h) = q.iter(world).next() {
        let mut by = h.damage_by.clone();
        by.sort_by(|a, b| b.1.total_cmp(&a.1));
        log.top = by
            .into_iter()
            .take(3)
            .map(|(spell, d)| {
                let name = data::ABILITIES
                    .iter()
                    .find(|a| a.spell == spell)
                    .map(|a| a.key.to_string())
                    .or_else(|| {
                        data::RUN_ITEMS
                            .iter()
                            .find(|i| i.spell == spell)
                            .map(|i| i.key.to_string())
                    })
                    .unwrap_or_else(|| spell.to_string());
                (name, d)
            })
            .collect();
    }
    log
}

/// What a stage should look like for a player new to the run, by stage (0-based): the most
/// seconds it may take, and the level range at its end.
pub(super) struct Band {
    pub max_secs: f32,
    pub levels: (u32, u32),
}

pub(super) const BANDS: [Band; 3] = [
    Band {
        max_secs: 760.0,
        levels: (14, 40),
    },
    Band {
        max_secs: 820.0,
        levels: (24, 60),
    },
    Band {
        max_secs: 880.0,
        levels: (32, 80),
    },
];

/// The run as a table row.
fn row(log: &RunLog) -> String {
    let stages: Vec<String> = log
        .stages
        .iter()
        .map(|s| {
            format!(
                "{:>4.0}s L{:<2} {:>5}k {:>6.0}dmg low{:>3.0}%{} portal {} called {} boss {}",
                s.secs,
                s.level,
                s.kills,
                s.taken,
                s.low * 100.0,
                if s.cleared { " ✓" } else { "  " },
                s.portal.map_or("-".into(), |t| format!("{t:.0}")),
                s.called.map_or("-".into(), |t| format!("{t:.0}")),
                s.boss_left
                    .map_or("-".into(), |b| format!("{:.0}%", b * 100.0)),
            )
        })
        .collect();
    let top: Vec<String> = log
        .top
        .iter()
        .map(|(n, d)| format!("{n} {:.0}k", d / 1000.0))
        .collect();
    format!(
        "{:<8} {:<5} {}  [{}]",
        log.class,
        if log.won {
            "won"
        } else if log.lost {
            "lost"
        } else {
            "-"
        },
        stages.join(" | "),
        top.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_run_plays_headless() {
        let log = play(0, 0, 1, 90.0);
        println!("{}", row(&log));
        let s = &log.stages[0];
        assert!(s.kills > 10, "the hero kills: {s:?}");
        assert!(s.level >= 3, "and levels: {s:?}");
        assert!(!log.lost, "and lives through the first minute and a half");
    }

    /// The first stage of every class, held to its band.
    #[test]
    #[ignore = "a few minutes: run with --ignored"]
    fn every_class_clears_its_first_stage_in_band() {
        for class in 0..data::CLASSES.len() {
            let log = play(class, 0, 7, BANDS[0].max_secs + 60.0);
            println!("{}", row(&log));
            let s = &log.stages[0];
            assert!(s.cleared, "{}: the first stage is cleared: {s:?}", log.class);
            assert!(s.secs <= BANDS[0].max_secs, "{}: {s:?}", log.class);
            let (lo, hi) = BANDS[0].levels;
            assert!((lo..=hi).contains(&s.level), "{}: {s:?}", log.class);
        }
    }

    /// One run's trace, to see what the pilot dies of.
    #[test]
    #[ignore = "a diagnosis: run with --ignored --nocapture"]
    fn trace_a_run() {
        let log = play(0, 0, 2, 900.0);
        println!("{}", row(&log));
        for p in &log.trace {
            println!(
                "{:>5.0}s hp {:>5.0} foes {:>3} near {:>2} took {:>4.0} (boss {:>4.0}){}",
                p.clock,
                p.hp,
                p.alive,
                p.near,
                p.taken,
                p.by_boss,
                if p.surge { " surge" } else { "" }
            );
        }
    }

    /// Every class through a whole run, the table printed.
    #[test]
    #[ignore = "long: run with --ignored --nocapture"]
    fn whole_runs_report() {
        let limit: f32 = BANDS.iter().map(|b| b.max_secs).sum::<f32>() + data::SWARM_SECS + 60.0;
        for class in 0..data::CLASSES.len() {
            for seed in [1, 2] {
                let log = play(class, 0, seed, limit);
                println!("{}", row(&log));
            }
        }
    }
}
