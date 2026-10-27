//! The run in stages, Megabonk-style: [`data::STAGES`] of them, each a zone of the campaign from
//! the one picked on, then the final swarm.
//!
//! - A stage's first bosses come on its clock; the final one does not. Once its time has come,
//!   the **boss portal** on the field opens: a hero standing in it calls the boss.
//! - The boss falls: the stage is cleared, and every hero gets gold, a key and a blessing to
//!   choose. A **portal onward** opens, and a hero standing in it takes everyone to the next
//!   stage: the next zone, foes tougher, field laid out anew. Every guest loads it before the
//!   clock runs again, as at a run's start.
//! - After the last stage's boss comes the **final swarm**: hold out [`data::SWARM_SECS`] to win.
//!
//! Only solo and the host tend this ([`tend_legs`], in `SimSet`). The portals reach guests as
//! [`Run::gates`], and the stage, its zone and the swarm on `Game::Run`; a guest follows the
//! zone and brings its own hero over itself.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::abilities::PendingHits;
use super::data;
use super::enemies::{self, Spawner};
use super::hero::Hero;
use super::treasure;
use super::units::{self, flat};
use super::{HeroState, Rng, Run, RunEntity, Selection, Session};

/// What a portal does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gate {
    /// Calls the stage's final boss.
    Boss,
    /// Takes everyone to the next stage.
    Onward,
}

impl Gate {
    pub fn to_wire(self) -> u8 {
        match self {
            Gate::Boss => 0,
            Gate::Onward => 1,
        }
    }

    pub fn from_wire(n: u8) -> Gate {
        if n == 0 {
            Gate::Boss
        } else {
            Gate::Onward
        }
    }
}

/// One portal as every machine sees it: which, where, how charged (0..1), and whether it may be
/// woken yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct GateView {
    pub kind: Gate,
    pub pos: Vec3,
    pub charge: f32,
    pub open: bool,
}

/// The stage's portals' models, as the host tends them, by [`Run::gates`] index.
#[derive(Resource, Default)]
pub(super) struct Legs {
    laid: Option<(u32, u8)>,
    models: Vec<Entity>,
}

fn spawn_gate(commands: &mut Commands, run: &mut Run, legs: &mut Legs, kind: Gate, pos: Vec3) {
    let (display, scale) = treasure::portal_look();
    let model = units::spawn_object(commands, run.guid(), display, scale, pos);
    legs.models.push(model);
    run.gates.push(GateView {
        kind,
        pos,
        charge: 0.0,
        open: kind == Gate::Onward,
    });
}

/// Charge the portals by who stands in them: the index of one charged full, if any. One left alone
/// ebbs at half speed; a closed one does not charge.
pub(super) fn charge_gates(gates: &mut [GateView], heroes: &[Vec3], dt: f32) -> Option<usize> {
    let mut woke = None;
    for (i, g) in gates.iter_mut().enumerate() {
        if !g.open {
            continue;
        }
        let inside = heroes
            .iter()
            .any(|h| flat(*h - g.pos).length() <= data::PORTAL_RADIUS);
        let step = dt / data::PORTAL_HOLD;
        g.charge = if inside {
            (g.charge + step).min(1.0)
        } else {
            (g.charge - step * 0.5).max(0.0)
        };
        if g.charge >= 1.0 && woke.is_none() {
            woke = Some(i);
        }
    }
    woke
}

/// What every hero gets as a stage ends: gold by the stage, a key, and three blessings to choose.
pub(super) fn reward(v: &mut super::Vitals, stage: u8, rng: &mut Rng) {
    let gold = data::STAGE_GOLD * (u32::from(stage) + 1);
    v.gold += gold as f32 * v.stats().gold;
    v.keys = (v.keys + 1).min(data::MAX_KEYS);
    if v.shrine_offer.is_empty() {
        v.shrine_offer = treasure::deal_boons(rng);
    }
    v.version += 1;
}

/// Lay out the stage's boss portal, open it on time, wake the portals heroes stand in, clear the
/// stage when its boss falls, run the final swarm, and take everyone onward.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn tend_legs(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    session: Res<Session>,
    mut selection: ResMut<Selection>,
    mut rng: ResMut<Rng>,
    mut legs: ResMut<Legs>,
    mut spawner: ResMut<Spawner>,
    mut pending: ResMut<PendingHits>,
    mut vacuum: ResMut<enemies::Vacuum>,
    world: WorldPoint,
    left: Query<Entity, (With<RunEntity>, Without<HeroState>)>,
    mut heroes: Query<(&mut Transform, &mut HeroState), With<Hero>>,
) {
    if !run.begun || run.won || run.lost {
        return;
    }
    let dt = time.delta_secs();
    let map = selection.stage_def();
    let center = super::hero::arena(map);
    let rules = run.threat(session.rules);

    // A new stage: its boss portal stands somewhere on the field, shut for now.
    if legs.laid != Some((run.run_id, run.leg)) {
        legs.laid = Some((run.run_id, run.leg));
        legs.models.clear();
        run.gates.clear();
        let a = rng.range(0.0, std::f32::consts::TAU);
        let pos = treasure::spot(&world, center, map.leash, a, 0.55);
        spawn_gate(&mut commands, &mut run, &mut legs, Gate::Boss, pos);
    }

    // The boss portal opens when the final boss's time has come.
    let due = rules.boss_time(data::BOSS_TIMES.len() - 1);
    if !run.leg_done && run.elapsed >= due {
        if let Some(g) = run
            .gates
            .iter_mut()
            .find(|g| g.kind == Gate::Boss && !g.open)
        {
            g.open = true;
            run.banner = Some((
                "The boss portal is open: stand in it to call the boss.".into(),
                4.0,
            ));
        }
    }

    // Heroes standing in a portal wake it.
    let standing: Vec<Vec3> = heroes
        .iter()
        .filter(|(_, h)| h.alive())
        .map(|(t, _)| t.translation)
        .collect();
    let mut gates = std::mem::take(&mut run.gates);
    let woke = charge_gates(&mut gates, &standing, dt);
    run.gates = gates;
    if let Some(i) = woke {
        let gate = run.gates.remove(i);
        if let Some(m) = (i < legs.models.len()).then(|| legs.models.remove(i)) {
            commands.entity(m).despawn();
        }
        match gate.kind {
            Gate::Boss => {
                let roster = data::roster(selection.stage);
                let last = data::BOSS_TIMES.len() as u32 - 1;
                let (def, final_boss) = rules.boss(roster, last);
                enemies::spawn_enemy(
                    &mut commands,
                    &mut run,
                    &rules,
                    def,
                    gate.pos,
                    center,
                    Some(last as u8),
                    final_boss,
                );
                run.bosses = run.bosses.max(last + 1);
                run.banner = Some((format!("{} has come: bring them down!", def.name), 5.0));
            }
            Gate::Onward => {
                // Everything of the stage goes but the heroes; the world turns to the next zone.
                for e in &left {
                    commands.entity(e).despawn();
                }
                run.leg += 1;
                selection.stage = data::stage_zone(selection.map, run.leg);
                let next = super::hero::arena(selection.stage_def());
                run.past_secs += run.elapsed;
                run.elapsed = 0.0;
                run.bosses = 0;
                run.rings = 0;
                run.final_slain = false;
                run.leg_done = false;
                run.gates.clear();
                legs.models.clear();
                *spawner = Spawner::default();
                pending.0.clear();
                // Everyone loads the new zone before its clock runs.
                run.waiting = session.multiplayer();
                for (k, (mut tf, mut h)) in heroes.iter_mut().enumerate() {
                    let a = k as f32 / 4.0 * std::f32::consts::TAU;
                    tf.translation = next + Vec3::new(a.cos(), 0.0, a.sin()) * 3.0;
                    let max = h.stats().max_hp;
                    h.hp = max;
                }
                run.banner = Some((
                    format!(
                        "Stage {} of {}: {}",
                        run.leg + 1,
                        data::STAGES,
                        selection.stage_def().name
                    ),
                    5.0,
                ));
                return;
            }
        }
    }

    // The stage's boss falls: rewards for everyone, and the way onward, or the final swarm.
    if run.final_slain && !run.leg_done {
        run.leg_done = true;
        // The field's gems all come in, so nothing is left behind on the way onward.
        vacuum.0 = data::VACUUM_SECS;
        let stage = run.leg;
        for (_, mut h) in &mut heroes {
            reward(&mut h.v, stage, &mut rng);
        }
        if run.leg + 1 < data::STAGES {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let pos = treasure::spot(&world, center, map.leash, a, 0.2);
            spawn_gate(&mut commands, &mut run, &mut legs, Gate::Onward, pos);
            run.banner = Some(("Stage cleared! A portal onward has opened.".into(), 5.0));
        } else {
            run.swarm = Some(data::SWARM_SECS);
            run.banner = Some(("The final swarm comes: hold out!".into(), 5.0));
        }
    }

    // The final swarm: held out, the run is won.
    if let Some(t) = run.swarm.as_mut() {
        *t -= dt;
        if *t <= 0.0 {
            run.swarm = None;
            run.cleared = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Vitals;

    fn gate(kind: Gate, open: bool) -> GateView {
        GateView {
            kind,
            pos: Vec3::ZERO,
            charge: 0.0,
            open,
        }
    }

    #[test]
    fn a_portal_charges_while_stood_in_and_ebbs_alone() {
        let mut gates = [gate(Gate::Boss, true)];
        let inside = [Vec3::X];
        let frames = (data::PORTAL_HOLD / 0.5).ceil() as usize;
        for _ in 0..frames - 1 {
            assert_eq!(charge_gates(&mut gates, &inside, 0.5), None);
        }
        assert_eq!(charge_gates(&mut gates, &inside, 0.5), Some(0));
        let mut gates = [GateView {
            charge: 0.5,
            ..gate(Gate::Onward, true)
        }];
        let away = [Vec3::X * (data::PORTAL_RADIUS + 1.0)];
        charge_gates(&mut gates, &away, 0.5);
        assert!(gates[0].charge < 0.5 && gates[0].charge > 0.0);
    }

    #[test]
    fn a_shut_portal_does_not_charge() {
        let mut gates = [gate(Gate::Boss, false)];
        for _ in 0..100 {
            assert_eq!(charge_gates(&mut gates, &[Vec3::ZERO], 1.0), None);
        }
        assert_eq!(gates[0].charge, 0.0);
    }

    #[test]
    fn a_stage_pays_more_the_further_along() {
        let mut rng = Rng::seeded(1);
        let mut first = Vitals::new(&data::CLASSES[0], 1);
        let mut third = first.clone();
        reward(&mut first, 0, &mut rng);
        reward(&mut third, 2, &mut rng);
        assert!(third.gold > first.gold);
        assert_eq!(first.keys, 1);
        assert_eq!(first.shrine_offer.len(), 3, "a blessing to choose");
    }

    #[test]
    fn each_stage_is_tougher_and_the_swarm_thicker() {
        let mut run = Run::default();
        let base = data::Rules::SOLO;
        let first = run.threat(base);
        run.leg = 2;
        let third = run.threat(base);
        assert!(third.health > first.health && third.damage > first.damage);
        run.swarm = Some(10.0);
        assert!(run.threat(base).density > third.density);
        // A stage runs a share of the run's clock.
        assert!((first.duration - base.duration * data::STAGE_LENGTH).abs() < 1e-3);
    }

    #[test]
    fn gates_cross_the_wire_by_kind() {
        for g in [Gate::Boss, Gate::Onward] {
            assert_eq!(Gate::from_wire(g.to_wire()), g);
        }
    }
}
