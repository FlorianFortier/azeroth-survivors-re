//! The host's half: it plays the run for every hero, and streams it to the guests as a server
//! streams a world. Each tick it sends every new unit with its update fields, the fields that
//! changed, every unit that moved, the units that went, and the casts, impacts, swings and
//! numbers [`Fx`](crate::survivors::units::Fx) played. Each guest gets its own hero's numbers.
//!
//! Guests send their own hero's motion, which the host takes as given within the battleground's
//! leash, and their card picks and mobility casts, which it checks against the rules.

use std::collections::{HashMap, HashSet};

use benilla_world::model_fade::DespawnFade;
use benilla_world::schedule::WorldStage;
use bevy::prelude::*;
use survivors_net::game::{
    AbilityView, Boon as WireBoon, Game, Gate as WireGate, Motion, ObjectKind, PartyEntry,
    Post as WirePost, Role, Score, Shrine as WireShrine, Spawn, Vitals as WireVitals,
    Ware as WireWare,
};
use survivors_net::protocol::Target;

use super::client::SEND_EVERY as GUEST_SEND_EVERY;
use super::smooth::{Clock, Track};
use super::{Net, Party, PartyMember};
use crate::creature_anim::MovementState;
use crate::net::{Guid, NetEntity, ObjectStore};
use crate::survivors::hero::{self, Hero};
use crate::survivors::mobility::MobilityQueue;
use crate::survivors::treasure::FieldMark;
use crate::survivors::units::{FxEvent, FxLog};
use crate::survivors::{
    data, sync, HeroState, Hostile, LocalHero, Mode, Pickup, Run, RunEntity, Selection, Session,
    SimSet, Vitals,
};

/// Seconds between stream ticks: the horde's motion and the frame's effects.
pub(super) const TICK: f32 = 0.05;
/// Seconds between the run's clock, the party and each guest's own numbers.
const SLOW_TICK: f32 = 0.2;
/// The host waits this long for every guest to load the zone before starting without them.
const LOAD_GRACE: f32 = 60.0;
/// Most effects carried between two ticks; past it, a frame's are dropped (the run is fine,
/// a guest only misses a few sparks).
const MAX_FX: usize = 1500;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (receive, smooth_guests)
            .chain()
            .after(WorldStage::Input)
            .before(SimSet)
            .run_if(is_host),
    )
    .add_systems(PostUpdate, stream.after(sync::sync_view).run_if(is_host));
}

fn is_host(session: Res<Session>) -> bool {
    session.mode == Mode::Host
}

struct Known {
    guid: u64,
    fields: HashMap<u16, u32>,
    motion: Motion,
    faded: bool,
}

/// What the guests already have, and the tick clocks.
#[derive(Resource, Default)]
pub(in crate::survivors) struct Replicator {
    known: HashMap<Entity, Known>,
    tick: f32,
    slow: f32,
    loaded: HashSet<u8>,
    wait: f32,
    /// Seconds the host's own zone has stood loaded.
    clear_for: f32,
    outcome_sent: bool,
    /// The stage the guests last loaded: a new one is waited for again.
    leg: u8,
    /// Each guest's clock against ours and the positions it sent for its hero, by seat.
    guests: HashMap<u8, (Clock, Track)>,
}

impl Replicator {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Make the next stream run both its ticks, whatever the clock says.
    #[cfg(test)]
    pub(super) fn due(&mut self) {
        self.tick = TICK;
        self.slow = SLOW_TICK;
    }
}

/// Each guest's hero, drawn [`super::smooth::DELAY`] behind its own clock, between the two
/// positions it sent around then: as smooth on the host's screen, and in the host's stream, as the
/// guest's own.
fn smooth_guests(
    time: Res<Time<Real>>,
    rep: Res<Replicator>,
    mut heroes: Query<(&mut Transform, &HeroState), (With<Hero>, Without<LocalHero>)>,
) {
    let now = time.elapsed_secs();
    for (mut tf, h) in &mut heroes {
        let Some((clock, track)) = rep.guests.get(&h.slot) else {
            continue;
        };
        let Some((pos, yaw)) = clock.draw_at(now).and_then(|at| track.at(at)) else {
            continue;
        };
        if tf.translation != pos {
            tf.translation = pos;
        }
        let want = Quat::from_rotation_y(yaw);
        if tf.rotation != want {
            tf.rotation = want;
        }
    }
}

fn motion_of(guid: u64, tf: &Transform, mv: Option<&MovementState>) -> Motion {
    let (yaw, _, _) = tf.rotation.to_euler(EulerRot::YXZ);
    Motion {
        guid,
        pos: tf.translation.to_array(),
        yaw,
        speed: mv.map_or(0.0, |m| m.speed),
        flags: mv.map_or(0, |m| m.flags),
        vertical_speed: mv.map_or(0.0, |m| m.vertical_speed),
    }
}

fn moved(a: &Motion, b: &Motion) -> bool {
    let d = Vec3::from_array(a.pos).distance_squared(Vec3::from_array(b.pos));
    d > 0.0004
        || (a.yaw - b.yaw).abs() > 0.01
        || a.speed != b.speed
        || a.flags != b.flags
        || a.vertical_speed != b.vertical_speed
}

/// A hero's numbers for its own player.
pub(super) fn wire_vitals(v: &Vitals) -> WireVitals {
    WireVitals {
        level: v.level,
        xp: v.xp,
        hp: v.hp,
        abilities: v
            .abilities
            .iter()
            .map(|o| AbilityView {
                key: o.def.key.to_string(),
                level: o.level,
                cd: o.cd,
                damage: o.perks.damage,
                cooldown: o.perks.cooldown,
                targets: o.perks.targets,
                area: o.perks.area,
                duration: o.perks.duration,
            })
            .collect(),
        passives: v
            .passives
            .iter()
            .map(|(p, l, power)| (p.key.to_string(), *l, *power))
            .collect(),
        shield: v.shield,
        shield_t: v.shield_t,
        immune_t: v.immune_t,
        thorns_t: v.thorns_t,
        heal_t: v.heal_t,
        pending_levels: v.pending_levels,
        dead_t: v.dead_t,
        damage_done: v.damage_done,
        kills: v.kills,
        picks: v.picks,
        revive_t: v.revive_t,
        loot: v.loot.iter().map(|i| i.name.clone()).collect(),
        gold: v.gold,
        run_items: v
            .run_items
            .iter()
            .map(|(i, n)| (i.key.to_string(), *n))
            .collect(),
        offer: v.offer.iter().map(|d| d.to_wire()).collect(),
        rerolls: v.rerolls,
        banishes: v.banishes,
        shrine_offer: v
            .shrine_offer
            .iter()
            .map(|b| WireBoon {
                bonus: data::bonus_to_wire(&b.bonus),
                rarity: b.rarity.to_wire(),
            })
            .collect(),
        boons: v.boons.iter().map(data::bonus_to_wire).collect(),
        chest_offer: v.chest_offer.iter().map(|i| i.key.to_string()).collect(),
        keys: v.keys,
        damage_by: v.damage_by.iter().map(|&(s, d)| (s, d as f32)).collect(),
    }
}

/// Guests' messages: their heroes' motion, cards and mobility; who has loaded; who has gone.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn receive(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut rep: ResMut<Replicator>,
    mut run: ResMut<Run>,
    session: Res<Session>,
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    selection: Res<Selection>,
    mut heroes: Query<
        (Entity, &mut Transform, &mut MovementState, &mut HeroState),
        (With<Hero>, Without<LocalHero>),
    >,
    mut queue: ResMut<MobilityQueue>,
    mut rng: ResMut<crate::survivors::Rng>,
    mut buys: ResMut<crate::survivors::market::BuyQueue>,
) {
    let inbox = std::mem::take(&mut net.inbox);
    let map = selection.stage_def();
    let center = hero::arena(map);
    for (from, g) in inbox {
        match g {
            Game::Loaded => {
                rep.loaded.insert(from);
            }
            Game::Input {
                t,
                motion,
                haste,
                haste_t,
                vanish_t,
            } => {
                let Some((_, tf, mut mv, mut h)) = heroes.iter_mut().find(|(.., h)| h.slot == from)
                else {
                    continue;
                };
                if !h.alive() || !motion.pos.iter().all(|v| v.is_finite()) {
                    continue;
                }
                // The guest moves its own hero; the host holds it to the battleground.
                let mut pos = Vec3::from_array(motion.pos);
                let off = Vec3::new(pos.x - center.x, 0.0, pos.z - center.z);
                if off.length() > map.leash {
                    let held = center + off.normalize() * map.leash;
                    pos.x = held.x;
                    pos.z = held.z;
                }
                // Drawn by `smooth_guests`, between two of the positions the guest sent.
                if t.is_finite() {
                    let yaw = if motion.yaw.is_finite() {
                        motion.yaw
                    } else {
                        tf.rotation.to_euler(EulerRot::YXZ).0
                    };
                    let (clock, track) = rep.guests.entry(from).or_default();
                    clock.sync(time.elapsed_secs(), t);
                    track.push(t, pos, yaw, GUEST_SEND_EVERY);
                }
                mv.speed = motion.speed.clamp(0.0, 40.0);
                mv.flags = motion.flags;
                mv.vertical_speed = motion.vertical_speed.clamp(-60.0, 60.0);
                // The guest's mobility is the guest's; only a learned one runs, and only so fast.
                if h.mobility().is_some() {
                    h.mobile.haste = haste.clamp(1.0, 2.0);
                    h.mobile.haste_t = haste_t.clamp(0.0, 8.0);
                    h.mobile.vanish_t = vanish_t.clamp(0.0, 8.0);
                }
            }
            Game::Buy { slot, item } => {
                // The market checks it: the hero at the merchant, the gold, the ware still there.
                buys.0.push((from, slot, item));
            }
            Game::ChestChoice(key) => {
                // Only an item of the offer this host's chest made that hero.
                if let Some((.., mut h)) = heroes.iter_mut().find(|(.., h)| h.slot == from) {
                    crate::survivors::treasure::take_chest_item(&mut h.v, &key);
                }
            }
            Game::ShrineChoice(bonus) => {
                // Only a blessing of the offer this host made that hero.
                if let Some((.., mut h)) = heroes.iter_mut().find(|(.., h)| h.slot == from) {
                    crate::survivors::treasure::take_boon(&mut h.v, bonus);
                }
            }
            Game::Card(action) => {
                // Checked against the offer this host dealt that hero, as a local move is.
                if let Some((.., mut h)) = heroes.iter_mut().find(|(.., h)| h.slot == from) {
                    crate::survivors::offer::apply_action(&mut h.v, &action, &mut rng);
                }
            }
            Game::Mobility { spell, effect } => {
                let Some((.., h)) = heroes.iter().find(|(.., h)| h.slot == from) else {
                    continue;
                };
                let allowed = h.alive() && h.mobility().is_some_and(|m| m.spell == spell);
                if allowed {
                    queue.0.push((from, spell, effect));
                }
            }
            // Host-bound only; anything else from a guest is ignored.
            _ => {}
        }
    }

    // A new stage is a new zone: everyone loads it again.
    if run.leg != rep.leg {
        rep.leg = run.leg;
        rep.loaded.clear();
        rep.wait = 0.0;
        rep.clear_for = 0.0;
    }
    // Everyone loads the zone before the clock runs; a guest who never does is not waited for.
    if run.waiting {
        let dt = time.delta_secs();
        rep.clear_for = if run.begun && !loading.covering() {
            rep.clear_for + dt
        } else {
            0.0
        };
        if rep.clear_for >= 1.0 {
            rep.loaded.insert(session.slot);
        }
        rep.wait += dt;
        // Everyone still at the table; one who left during the load is not waited for.
        let seated: Vec<u8> = net
            .lobby
            .as_ref()
            .map_or_else(Vec::new, |l| l.members.iter().map(|m| m.slot).collect());
        let all = seated.iter().all(|s| rep.loaded.contains(s));
        if all || rep.wait > LOAD_GRACE {
            run.waiting = false;
        }
    }

    // A guest who left takes their hero with them.
    if let Some(lobby) = &net.lobby {
        for (e, .., h) in &heroes {
            if !lobby.members.iter().any(|m| m.slot == h.slot) {
                commands
                    .entity(e)
                    .remove::<HeroState>()
                    .remove::<Hero>()
                    .insert(DespawnFade::default());
            }
        }
    }
}

/// One stream tick to every guest.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn stream(
    mut net: ResMut<Net>,
    mut rep: ResMut<Replicator>,
    mut party: ResMut<Party>,
    time: Res<Time<Real>>,
    run: Res<Run>,
    session: Res<Session>,
    selection: Res<Selection>,
    objects: Query<
        (
            Entity,
            &Guid,
            &NetEntity,
            &ObjectStore,
            &Transform,
            Option<&MovementState>,
            Option<&HeroState>,
            Has<Hostile>,
            Has<Pickup>,
            Has<DespawnFade>,
            Option<&FieldMark>,
        ),
        With<RunEntity>,
    >,
    guids: Query<&Guid>,
    log: Option<ResMut<FxLog>>,
) {
    let dt = time.delta_secs();
    rep.tick += dt;
    rep.slow += dt;
    if rep.tick < TICK || !run.begun {
        return;
    }
    rep.tick = 0.0;

    let mut out: Vec<Game> = Vec::new();
    let mut motions: Vec<Motion> = Vec::new();
    let mut seen: HashSet<Entity> = HashSet::with_capacity(rep.known.len());

    for (e, guid, net_entity, store, tf, mv, hero_state, hostile, pickup, fading, mark) in &objects
    {
        seen.insert(e);
        if fading {
            if let Some(k) = rep.known.get_mut(&e) {
                if !k.faded {
                    k.faded = true;
                    out.push(Game::Despawn {
                        guid: k.guid,
                        fade: true,
                    });
                }
            }
            continue;
        }
        let kind = match net_entity.kind {
            benilla_protocol::EntityKind::GameObject => ObjectKind::GameObject,
            benilla_protocol::EntityKind::DynamicObject => ObjectKind::DynamicObject,
            benilla_protocol::EntityKind::Player => ObjectKind::Player,
            _ => ObjectKind::Unit,
        };
        let now = motion_of(guid.0, tf, mv);
        match rep.known.get_mut(&e) {
            None => {
                let role = match (hero_state, mark, hostile, pickup, kind) {
                    (Some(h), ..) => Role::Hero(h.slot),
                    (_, Some(FieldMark::Chest), ..) => Role::Chest,
                    (_, Some(FieldMark::FreeChest), ..) => Role::FreeChest,
                    (_, Some(FieldMark::Merchant), ..) => Role::Merchant,
                    (_, Some(FieldMark::Elite(k)), ..) => Role::Elite(*k),
                    (_, _, true, ..) => Role::Enemy,
                    (_, _, _, true, _) => Role::Pickup,
                    (.., ObjectKind::DynamicObject) => Role::Area,
                    (.., ObjectKind::GameObject) => Role::Pickup,
                    _ => Role::Totem,
                };
                let fields: Vec<(u16, u32)> = store.0.raw_fields().collect();
                out.push(Game::Spawn(Spawn {
                    guid: guid.0,
                    kind,
                    role,
                    display: net_entity.display_id,
                    scale: net_entity.scale,
                    fields: fields.clone(),
                    pos: now.pos,
                    yaw: now.yaw,
                }));
                rep.known.insert(
                    e,
                    Known {
                        guid: guid.0,
                        fields: fields.into_iter().collect(),
                        motion: now,
                        faded: false,
                    },
                );
            }
            Some(k) => {
                let mut changes: Vec<(u16, u32)> = Vec::new();
                for (i, v) in store.0.raw_fields() {
                    if k.fields.get(&i) != Some(&v) {
                        changes.push((i, v));
                        k.fields.insert(i, v);
                    }
                }
                if !changes.is_empty() {
                    out.push(Game::Fields {
                        guid: k.guid,
                        changes,
                    });
                }
                if moved(&k.motion, &now) {
                    k.motion = now;
                    motions.push(now);
                }
            }
        }
    }

    // Gone without a fade: a collected gem, a burst area, a totem's end.
    let gone: Vec<Entity> = rep
        .known
        .keys()
        .filter(|e| !seen.contains(e))
        .copied()
        .collect();
    for e in gone {
        if let Some(k) = rep.known.remove(&e) {
            if !k.faded {
                out.push(Game::Despawn {
                    guid: k.guid,
                    fade: false,
                });
            }
        }
    }

    if !motions.is_empty() {
        out.push(Game::Motions {
            t: run.elapsed,
            list: motions,
        });
    }

    // The frame's effects, by guid.
    if let Some(mut log) = log {
        let events = std::mem::take(&mut log.0);
        let skip = events.len().saturating_sub(MAX_FX);
        let g = |e: Entity| guids.get(e).ok().map(|g| g.0);
        for ev in events.into_iter().skip(skip) {
            match ev {
                FxEvent::Cast {
                    caster,
                    spell,
                    hits,
                    dest,
                } => {
                    if let Some(caster) = g(caster) {
                        out.push(Game::Cast {
                            caster,
                            spell,
                            hits: hits.into_iter().filter_map(g).collect(),
                            dest: dest.map(|d| d.to_array()),
                        });
                    }
                }
                FxEvent::Impact { target, spell } => {
                    if let Some(target) = g(target) {
                        out.push(Game::Impact { target, spell });
                    }
                }
                FxEvent::Swing {
                    attacker,
                    victim,
                    damage,
                } => {
                    if let (Some(attacker), Some(victim)) = (g(attacker), g(victim)) {
                        out.push(Game::Swing {
                            attacker,
                            victim,
                            damage: damage.max(0.0) as u32,
                        });
                    }
                }
                FxEvent::Number {
                    anchor,
                    amount,
                    crit,
                    color,
                } => {
                    if let Some(anchor) = g(anchor) {
                        out.push(Game::Number {
                            anchor,
                            amount,
                            crit,
                            color,
                        });
                    }
                }
            }
        }
    }

    // The run's clock, the party, each guest's own numbers, and the outcome.
    let heroes: Vec<(u64, &HeroState)> = objects
        .iter()
        .filter_map(|(_, g, .., h, _, _, _, _)| h.map(|h| (g.0, h)))
        .collect();
    if rep.slow >= SLOW_TICK {
        rep.slow = 0.0;
        out.push(Game::Run {
            elapsed: run.elapsed,
            bosses: run.bosses,
            waiting: run.waiting,
            banner: run.banner.clone(),
            chest_cost: run.chest_cost,
            posts: run
                .posts
                .iter()
                .map(|p| WirePost {
                    kind: p.kind as u8,
                    pos: p.pos.to_array(),
                    wait: p.wait,
                })
                .collect(),
            shrines: run
                .shrines
                .iter()
                .map(|s| WireShrine {
                    kind: s.kind as u8,
                    pos: s.pos.to_array(),
                    charge: s.charge,
                    state: s.state.to_wire(),
                })
                .collect(),
            greed: run.greed,
            curse: run.curse,
            shop: run
                .shop
                .iter()
                .map(|w| WireWare {
                    item: w.item.key.to_string(),
                    price: w.price,
                    sold: w.sold,
                })
                .collect(),
            stage: run.leg,
            zone: selection.stage.min(usize::from(u8::MAX)) as u8,
            gates: run
                .gates
                .iter()
                .map(|g| WireGate {
                    kind: g.kind.to_wire(),
                    pos: g.pos.to_array(),
                    charge: g.charge,
                    open: g.open,
                })
                .collect(),
            swarm: run.swarm.unwrap_or(-1.0),
        });
        let entries: Vec<PartyEntry> = heroes
            .iter()
            .map(|(guid, h)| PartyEntry {
                slot: h.slot,
                guid: *guid,
                level: h.level,
                hp: h.hp,
                max_hp: h.stats().max_hp,
                dead: !h.alive(),
            })
            .collect();
        out.push(Game::Party(entries));
        party.members = heroes
            .iter()
            .map(|(_, h)| PartyMember {
                slot: h.slot,
                name: session.name_of(h.slot).to_string(),
                class: crate::survivors::data::CLASSES
                    .iter()
                    .position(|c| c.id == h.class.id)
                    .unwrap_or(0),
                level: h.level,
                hp: h.hp,
                max_hp: h.stats().max_hp,
                dead: !h.alive(),
                kills: h.kills,
                damage_done: h.damage_done,
            })
            .collect();
        for (_, h) in &heroes {
            if h.slot != session.slot {
                net.relay(Target::Slot(h.slot), &[Game::Me(wire_vitals(&h.v))]);
            }
        }
    }
    if (run.won || run.lost) && !rep.outcome_sent {
        rep.outcome_sent = true;
        out.push(Game::Outcome {
            won: run.won,
            scores: heroes
                .iter()
                .map(|(_, h)| Score {
                    slot: h.slot,
                    level: h.level,
                    kills: h.kills,
                    damage_done: h.damage_done,
                })
                .collect(),
        });
    }

    net.relay(Target::Others, &out);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_moves_are_not_resent() {
        let a = Motion {
            guid: 1,
            pos: [0.0, 0.0, 0.0],
            yaw: 0.0,
            speed: 7.0,
            flags: 1,
            vertical_speed: 0.0,
        };
        let mut b = a;
        b.pos[0] = 0.01;
        assert!(!moved(&a, &b));
        b.pos[0] = 0.5;
        assert!(moved(&a, &b));
        let mut c = a;
        c.flags = 0;
        assert!(moved(&a, &c));
    }

    #[test]
    fn a_heros_numbers_cross_the_wire_by_key() {
        let class = &crate::survivors::data::CLASSES[0];
        let v = Vitals::new(class, 1);
        let w = wire_vitals(&v);
        assert_eq!(w.abilities.len(), 1);
        assert_eq!(w.abilities[0].key, class.start);
        assert_eq!(w.hp, v.hp);
    }
}
