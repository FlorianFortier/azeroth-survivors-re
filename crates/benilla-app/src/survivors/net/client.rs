//! A guest's half: it builds the host's units exactly as the net layer builds a server's — a
//! [`Guid`], a [`NetEntity`] and an [`ObjectStore`] are all the attach, animation and spell-visual
//! systems need — and plays the host's casts, impacts, swings and numbers through the same
//! [`Fx`]. Everything is drawn a moment behind the host, between two positions it really sent
//! ([`super::smooth`]). The guest's own hero is the one exception: this machine moves it, and
//! sends its motion, cards and mobility to the host.

use std::collections::HashMap;

use benilla_protocol::messages::{ObjectFields, ObjectType};
use benilla_world::model_fade::DespawnFade;
use benilla_world::schedule::WorldStage;
use bevy::prelude::*;
use survivors_net::game::{Game, Motion, ObjectKind, Role, Spawn, Vitals as WireVitals};
use survivors_net::protocol::Target;

use super::host::TICK as HOST_TICK;
use super::smooth::{Clock, Track};
use super::{Net, Party, PartyMember};
use crate::creature_anim::MovementState;
use crate::net::{FieldChanged, Guid, NetEntity, ObjectStore, ScriptedFacing};
use crate::survivors::hero::Hero;
use crate::survivors::mobility::{self, MobilityQueue};
use crate::survivors::offer::Dealt as PostDealt;
use crate::survivors::sync::{self, LocalPicks};
use crate::survivors::treasure::{FieldMark, PostView, ShrineState, ShrineView};
use crate::survivors::units::Fx;
use crate::survivors::{data, Hostile, LocalHero, Mode, Owned, Pickup, Run, RunEntity, Session};

/// Seconds between the guest's own hero updates to the host.
pub(super) const SEND_EVERY: f32 = 0.05;
/// Seconds the zone must stand loaded before the guest reports it.
const LOADED_AFTER: f32 = 1.0;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        Update,
        (rejoined, receive, arrive, interpolate)
            .chain()
            .after(WorldStage::Input)
            .run_if(is_client),
    )
    .add_systems(PostUpdate, send.after(sync::sync_view).run_if(is_client));
}

fn is_client(session: Res<Session>) -> bool {
    session.mode == Mode::Client
}

/// The host's objects by guid, and the guest's own bookkeeping.
#[derive(Resource, Default)]
pub(in crate::survivors) struct Replicas {
    by_guid: HashMap<u64, Entity>,
    /// The host's clock against ours, from its motion ticks; and its latest tick's time.
    clock: Clock,
    host_t: f32,
    tick: f32,
    loaded_sent: bool,
    /// Seconds the zone has stood loaded: the loading screen can take a frame to rise.
    clear_for: f32,
    /// The run's stage this guest stands in; a new one takes its hero over to it.
    leg: u8,
    /// Where the local hero goes, on to a new stage.
    arrive: Option<Vec3>,
}

impl Replicas {
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// A copy of a host unit, and the positions the host sent for it.
#[derive(Component)]
pub(super) struct Replica {
    track: Track,
}

impl Replica {
    /// The newest position the host sent.
    #[cfg(test)]
    pub(super) fn newest(&self) -> Option<Vec3> {
        self.track.at(f32::MAX).map(|(p, _)| p)
    }
}

/// A point the host sent, if it is one: a corrupt or hostile frame's NaN or infinity never
/// reaches a transform.
fn finite(p: [f32; 3]) -> Option<Vec3> {
    p.iter().all(|v| v.is_finite()).then(|| Vec3::from_array(p))
}

/// An angle the host sent, or none.
fn finite_or(v: f32, or: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        or
    }
}

fn object_type(kind: ObjectKind) -> ObjectType {
    match kind {
        ObjectKind::Unit => ObjectType::Unit,
        ObjectKind::Player => ObjectType::Player,
        ObjectKind::GameObject => ObjectType::GameObject,
        ObjectKind::DynamicObject => ObjectType::DynamicObject,
    }
}

fn spawn_copy(commands: &mut Commands, s: &Spawn, me: u8, host_t: f32) -> Entity {
    let pos = Vec3::from_array(s.pos);
    let kind = match s.kind {
        ObjectKind::Unit => benilla_protocol::EntityKind::Unit,
        ObjectKind::GameObject => benilla_protocol::EntityKind::GameObject,
        ObjectKind::DynamicObject => benilla_protocol::EntityKind::DynamicObject,
        ObjectKind::Player => benilla_protocol::EntityKind::Player,
    };
    // A player hero is dressed from its fields: its pieces' templates are answered here.
    if s.kind == ObjectKind::Player {
        crate::survivors::outfit::queue_templates(
            commands,
            crate::survivors::outfit::worn_entries(&s.fields),
        );
    }
    let mut ec = commands.spawn((
        Guid(s.guid),
        NetEntity {
            kind,
            display_id: s.display,
            scale: s.scale,
        },
        ObjectStore(ObjectFields::from_pairs(&s.fields)),
        Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(s.yaw)),
        Visibility::default(),
        RunEntity,
    ));
    if matches!(s.kind, ObjectKind::Unit | ObjectKind::Player) {
        ec.insert((MovementState::default(), ScriptedFacing));
    }
    // Its first position at the host's latest tick, for its first step to start from.
    let mut track = Track::default();
    track.push(host_t, pos, s.yaw, HOST_TICK);
    let copy = Replica { track };
    match s.role {
        Role::Hero(slot) if slot == me => {
            ec.insert((Hero::default(), LocalHero));
        }
        Role::Hero(_) => {
            ec.insert((Hero::default(), copy));
        }
        Role::Enemy => {
            ec.insert((Hostile, copy));
        }
        Role::Pickup => {
            ec.insert((Pickup, copy));
        }
        Role::Chest => {
            ec.insert((Pickup, FieldMark::Chest, copy));
        }
        Role::FreeChest => {
            ec.insert((Pickup, FieldMark::FreeChest, copy));
        }
        Role::Merchant => {
            ec.insert((FieldMark::Merchant, copy));
        }
        Role::Elite(kind) => {
            ec.insert((Hostile, FieldMark::Elite(kind), copy));
        }
        Role::Totem | Role::Area => {
            ec.insert(copy);
        }
    }
    ec.id()
}

/// The host's numbers for this guest's hero, onto the HUD's view. Cards this guest picked that the
/// host has not applied yet stay as the view shows them.
fn apply_me(run: &mut Run, v: WireVitals) {
    let ahead = v.picks < run.picks;
    let was_level = run.level;
    run.level = v.level;
    run.xp = v.xp;
    run.hp = v.hp;
    run.shield = v.shield;
    run.shield_t = v.shield_t;
    run.immune_t = v.immune_t;
    run.thorns_t = v.thorns_t;
    run.heal_t = v.heal_t;
    run.dead_t = v.dead_t;
    run.revive_t = v.revive_t;
    run.damage_done = v.damage_done;
    run.kills = v.kills;
    run.gold = v.gold;
    run.keys = v.keys;
    run.damage_by = v
        .damage_by
        .iter()
        .map(|&(s, d)| (s, f64::from(d)))
        .collect();
    let items: Vec<_> = v
        .run_items
        .iter()
        .filter_map(|(k, n)| data::run_item(k).map(|i| (i, *n)))
        .collect();
    if items != run.run_items {
        // The items row and its tooltips rebuild on the version.
        run.run_items = items;
        run.version += 1;
    }
    if run.loot.len() != v.loot.len() {
        run.loot = v.loot.iter().filter_map(|n| data::item(n)).collect();
    }
    if ahead {
        // Only the cooldowns, on the abilities both sides know.
        for a in &v.abilities {
            if let Some(o) = run.abilities.iter_mut().find(|o| o.def.key == a.key) {
                o.cd = a.cd;
            }
        }
    } else {
        let abilities: Vec<Owned> = v
            .abilities
            .iter()
            .filter_map(|a| {
                data::ABILITIES
                    .iter()
                    .find(|d| d.key == a.key)
                    .map(|def| Owned {
                        def,
                        level: a.level,
                        cd: a.cd,
                        perks: data::Perks {
                            damage: a.damage,
                            cooldown: a.cooldown,
                            targets: a.targets,
                            area: a.area,
                            duration: a.duration,
                        },
                    })
            })
            .collect();
        let passives: Vec<_> = v
            .passives
            .iter()
            .filter_map(|(k, l, power)| {
                data::PASSIVES
                    .iter()
                    .find(|p| p.key == k)
                    .map(|p| (p, *l, *power))
            })
            .collect();
        let same = abilities.len() == run.abilities.len()
            && abilities
                .iter()
                .zip(&run.abilities)
                .all(|(a, b)| a.def.key == b.def.key && a.level == b.level)
            && passives.len() == run.passives.len()
            && passives
                .iter()
                .zip(&run.passives)
                .all(|(a, b)| a.0.key == b.0.key && a.1 == b.1);
        run.abilities = abilities;
        run.passives = passives;
        run.pending_levels = v.pending_levels;
        run.picks = v.picks;
        // The cards the host dealt, and what is left to play them with.
        run.offer = v.offer.iter().filter_map(PostDealt::from_wire).collect();
        run.rerolls = v.rerolls;
        run.banishes = v.banishes;
        // The blessings offered, and what the shrines gave.
        run.shrine_offer = v
            .shrine_offer
            .iter()
            .filter_map(|b| {
                Some(data::Boon {
                    bonus: data::bonus_from_wire(b.bonus)?,
                    rarity: data::CardRarity::from_wire(b.rarity)?,
                })
            })
            .collect();
        let boons: Vec<_> = v
            .boons
            .iter()
            .filter_map(|&b| data::bonus_from_wire(b))
            .collect();
        if boons != run.boons {
            run.boons = boons;
            run.version += 1;
        }
        if !same {
            run.version += 1;
        }
    }
    if was_level < mobility::LEARN_LEVEL && run.level >= mobility::LEARN_LEVEL {
        if let Some(m) = mobility::for_class(run.class.id) {
            run.banner = Some((format!("You have learned {}! Press F.", m.name), 4.0));
        }
    }
}

/// The host's stream, applied.
#[allow(clippy::too_many_arguments)]
pub(super) fn receive(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut reps: ResMut<Replicas>,
    mut run: ResMut<Run>,
    session: Res<Session>,
    mut selection: ResMut<crate::survivors::Selection>,
    mut party: ResMut<Party>,
    mut fx: Fx,
    mut movers: Query<(&mut Replica, Option<&mut MovementState>)>,
    time: Res<Time<Real>>,
) {
    if !run.begun {
        return;
    }
    let inbox = std::mem::take(&mut net.inbox);
    let get = |reps: &Replicas, g: u64| reps.by_guid.get(&g).copied();
    for (from, g) in inbox {
        // Only the host speaks for the run.
        if from != 0 {
            continue;
        }
        match g {
            Game::Spawn(mut s) => {
                if finite(s.pos).is_none() {
                    continue;
                }
                s.yaw = finite_or(s.yaw, 0.0);
                if let Some(old) = reps.by_guid.remove(&s.guid) {
                    commands.entity(old).try_despawn();
                }
                let e = spawn_copy(&mut commands, &s, session.slot, reps.host_t);
                reps.by_guid.insert(s.guid, e);
            }
            Game::Fields { guid, changes } => {
                let Some(e) = get(&reps, guid) else { continue };
                // Through the command queue, so a copy spawned this same frame still takes it.
                commands.queue(move |world: &mut World| {
                    let mut edges: Vec<FieldChanged> = Vec::new();
                    if let Some(mut store) = world.get_mut::<ObjectStore>(e) {
                        let kind = store.0.object_type().unwrap_or(ObjectType::Unit);
                        store.0.merge_diff(
                            ObjectFields::from_pairs(&changes),
                            |index, old, new| {
                                edges.push(FieldChanged {
                                    entity: e,
                                    guid,
                                    kind,
                                    index,
                                    old,
                                    new,
                                });
                            },
                        );
                    }
                    for edge in edges {
                        world.write_message(edge);
                    }
                });
            }
            Game::Motions { t, list } => {
                if !t.is_finite() {
                    continue;
                }
                reps.clock.sync(time.elapsed_secs(), t);
                reps.host_t = t;
                for m in list {
                    let Some(e) = get(&reps, m.guid) else {
                        continue;
                    };
                    let Some(pos) = finite(m.pos) else { continue };
                    let yaw = finite_or(m.yaw, 0.0);
                    if let Ok((mut copy, mv)) = movers.get_mut(e) {
                        copy.track.push(t, pos, yaw, HOST_TICK);
                        if let Some(mut mv) = mv {
                            mv.speed = m.speed;
                            mv.flags = m.flags;
                            mv.vertical_speed = m.vertical_speed;
                        }
                    } else {
                        // A copy spawned this same frame (a burst brought its spawn and its first
                        // steps together) is not in the world yet: its steps follow its spawn
                        // through the command queue, or the copy would stand where it came in.
                        commands.queue(move |world: &mut World| {
                            if let Some(mut copy) = world.get_mut::<Replica>(e) {
                                copy.track.push(t, pos, yaw, HOST_TICK);
                            }
                            if let Some(mut mv) = world.get_mut::<MovementState>(e) {
                                mv.speed = m.speed;
                                mv.flags = m.flags;
                                mv.vertical_speed = m.vertical_speed;
                            }
                        });
                    }
                }
            }
            Game::Despawn { guid, fade } => {
                if let Some(e) = reps.by_guid.remove(&guid) {
                    if fade {
                        commands
                            .entity(e)
                            .remove::<Replica>()
                            .try_insert(DespawnFade::default());
                    } else {
                        commands.entity(e).try_despawn();
                    }
                }
            }
            Game::Cast {
                caster,
                spell,
                hits,
                dest,
            } => {
                let Some(c) = get(&reps, caster) else {
                    continue;
                };
                let hits: Vec<Entity> = hits.iter().filter_map(|h| get(&reps, *h)).collect();
                fx.cast(c, spell, &hits, dest.and_then(finite));
            }
            Game::Impact { target, spell } => {
                if let Some(t) = get(&reps, target) {
                    fx.impact(t, spell);
                }
            }
            Game::Swing {
                attacker,
                victim,
                damage,
            } => {
                if let (Some(a), Some(v)) = (get(&reps, attacker), get(&reps, victim)) {
                    fx.swing(a, v, damage as f32);
                }
            }
            Game::Number {
                anchor,
                amount,
                crit,
                color,
            } => {
                if let (Some(a), true) = (get(&reps, anchor), amount.is_finite()) {
                    fx.number(a, amount, crit, color);
                }
            }
            Game::Run {
                elapsed,
                bosses,
                waiting,
                banner,
                chest_cost,
                posts,
                shrines,
                greed,
                curse,
                shop,
                stage,
                zone,
                gates,
                swarm,
            } => {
                // On to a new stage: the world follows the zone, the hero goes over, and the
                // zone is loaded again before the host's clock runs.
                if stage != reps.leg {
                    reps.leg = stage;
                    run.past_secs += run.elapsed;
                    selection.stage = usize::from(zone).min(data::MAPS.len() - 1);
                    let center = crate::survivors::hero::arena(selection.stage_def());
                    let a = f32::from(session.slot) / 4.0 * std::f32::consts::TAU;
                    reps.arrive = Some(center + Vec3::new(a.cos(), 0.0, a.sin()) * 3.0);
                    reps.loaded_sent = false;
                    reps.clear_for = 0.0;
                }
                run.leg = stage;
                run.gates = gates
                    .into_iter()
                    .filter_map(|g| {
                        Some(crate::survivors::legs::GateView {
                            kind: crate::survivors::legs::Gate::from_wire(g.kind),
                            pos: finite(g.pos)?,
                            charge: finite_or(g.charge, 0.0),
                            open: g.open,
                        })
                    })
                    .collect();
                run.swarm = (swarm >= 0.0).then_some(swarm);
                run.shop = shop
                    .into_iter()
                    .filter_map(|w| {
                        Some(crate::survivors::market::Ware {
                            item: data::run_item(&w.item)?,
                            price: w.price,
                            sold: w.sold,
                        })
                    })
                    .collect();
                run.shrines = shrines
                    .into_iter()
                    .filter_map(|s| {
                        Some(ShrineView {
                            kind: usize::from(s.kind),
                            pos: finite(s.pos)?,
                            charge: finite_or(s.charge, 0.0),
                            state: ShrineState::from_wire(s.state),
                        })
                    })
                    .collect();
                run.greed = greed;
                run.curse = curse;
                run.chest_cost = chest_cost;
                run.posts = posts
                    .into_iter()
                    .filter_map(|p| {
                        Some(PostView {
                            kind: usize::from(p.kind),
                            pos: finite(p.pos)?,
                            wait: finite_or(p.wait, 0.0),
                        })
                    })
                    .collect();
                run.elapsed = finite_or(elapsed, run.elapsed);
                run.bosses = bosses;
                run.waiting = waiting;
                if banner.is_some() {
                    run.banner = banner;
                }
            }
            Game::Me(v) => apply_me(&mut run, v),
            Game::Party(list) => {
                party.members = list
                    .into_iter()
                    .map(|p| {
                        let old = party.members.iter().find(|m| m.slot == p.slot);
                        PartyMember {
                            slot: p.slot,
                            name: session.name_of(p.slot).to_string(),
                            class: session
                                .seats
                                .iter()
                                .find(|s| s.slot == p.slot)
                                .map_or(0, |s| s.class),
                            level: p.level,
                            hp: p.hp,
                            max_hp: p.max_hp,
                            dead: p.dead,
                            kills: old.map_or(0, |o| o.kills),
                            damage_done: old.map_or(0.0, |o| o.damage_done),
                        }
                    })
                    .collect();
            }
            Game::Outcome { won, scores } => {
                run.won = won;
                run.lost = !won;
                for s in scores {
                    if let Some(m) = party.members.iter_mut().find(|m| m.slot == s.slot) {
                        m.kills = s.kills;
                        m.damage_done = s.damage_done;
                        m.level = s.level;
                    }
                }
            }
            // Guest-to-host messages are never relayed here.
            Game::Loaded
            | Game::Input { .. }
            | Game::Card(_)
            | Game::ShrineChoice(_)
            | Game::Buy { .. }
            | Game::Mobility { .. }
            | Game::Resync => {}
        }
    }
}

/// Back in the run after a dropped connection: everything the copy held is stale, so it goes,
/// the guest's own hero with it, and the host is asked for the whole world again, which its next
/// tick sends (`host::stream`).
pub(super) fn rejoined(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut reps: ResMut<Replicas>,
    mut run: ResMut<Run>,
) {
    // While it tries, the run says so.
    if net.rejoin.is_some() {
        run.banner = Some(("Connection lost: rejoining…".into(), 0.5));
    }
    if !std::mem::take(&mut net.resync) {
        return;
    }
    for (_, e) in reps.by_guid.drain() {
        commands.entity(e).try_despawn();
    }
    // The stage and the load stand as they were; the host's clock is met anew.
    let leg = reps.leg;
    let loaded_sent = reps.loaded_sent;
    *reps = Replicas {
        leg,
        loaded_sent,
        ..Replicas::default()
    };
    net.inbox.clear();
    net.relay(Target::Host, &[Game::Resync]);
    run.banner = Some(("You are back in the run.".into(), 3.0));
}

/// The local hero steps over to a new stage's battleground.
pub(super) fn arrive(
    mut reps: ResMut<Replicas>,
    mut me: Query<&mut Transform, (With<LocalHero>, Without<Replica>)>,
) {
    let Some(at) = reps.arrive.take() else { return };
    if let Ok(mut tf) = me.single_mut() {
        tf.translation = at;
    }
}

/// Copies are drawn [`super::smooth::DELAY`] behind the host, between the two positions around
/// then.
fn interpolate(
    time: Res<Time<Real>>,
    reps: Res<Replicas>,
    mut copies: Query<(&mut Transform, &Replica)>,
) {
    let Some(at) = reps.clock.draw_at(time.elapsed_secs()) else {
        return;
    };
    for (mut tf, copy) in &mut copies {
        let Some((pos, yaw)) = copy.track.at(at) else {
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

/// The guest's own hero, cards and mobility, to the host.
#[allow(clippy::too_many_arguments)]
fn send(
    mut net: ResMut<Net>,
    mut reps: ResMut<Replicas>,
    run: Res<Run>,
    time: Res<Time<Real>>,
    loading: Res<crate::loading_screen::LoadingScreen>,
    mut picks: ResMut<LocalPicks>,
    mut buys: ResMut<crate::survivors::market::BuyQueue>,
    mut queue: ResMut<MobilityQueue>,
    me: Query<(&Guid, &Transform, Option<&MovementState>), With<LocalHero>>,
) {
    let mut out: Vec<Game> = Vec::new();
    reps.clear_for = if run.begun && !loading.covering() {
        reps.clear_for + time.delta_secs()
    } else {
        0.0
    };
    if !reps.loaded_sent && reps.clear_for >= LOADED_AFTER {
        reps.loaded_sent = true;
        out.push(Game::Loaded);
    }
    out.extend(picks.0.drain(..).map(Game::Card));
    out.extend(picks.1.drain(..).map(Game::ShrineChoice));
    out.extend(
        buys.0
            .drain(..)
            .map(|(_, slot, item)| Game::Buy { slot, item }),
    );
    out.extend(
        queue
            .0
            .drain(..)
            .map(|(_, spell, effect)| Game::Mobility { spell, effect }),
    );
    reps.tick += time.delta_secs();
    if reps.tick >= SEND_EVERY {
        reps.tick = 0.0;
        if let Ok((guid, tf, mv)) = me.single() {
            let (yaw, _, _) = tf.rotation.to_euler(EulerRot::YXZ);
            out.push(Game::Input {
                t: time.elapsed_secs(),
                motion: Motion {
                    guid: guid.0,
                    pos: tf.translation.to_array(),
                    yaw,
                    speed: mv.map_or(0.0, |m| m.speed),
                    flags: mv.map_or(0, |m| m.flags),
                    vertical_speed: mv.map_or(0.0, |m| m.vertical_speed),
                },
                haste: run.mobile.haste,
                haste_t: run.mobile.haste_t,
                vanish_t: run.mobile.vanish_t,
            });
        }
    }
    net.relay(Target::Host, &out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Vitals;
    use survivors_net::game::AbilityView;

    fn view() -> Run {
        let class = &data::CLASSES[0];
        Run {
            me: Vitals::new(class, 1),
            begun: true,
            ..Run::default()
        }
    }

    #[test]
    fn the_hosts_numbers_land_on_the_view() {
        let mut run = view();
        let key = run.abilities[0].def.key.to_string();
        let before = run.version;
        apply_me(
            &mut run,
            WireVitals {
                level: 7,
                hp: 50.0,
                abilities: vec![AbilityView {
                    key: key.clone(),
                    level: 3,
                    cd: 0.7,
                    damage: 0.0,
                    cooldown: 1.0,
                    targets: 0,
                    area: 0.0,
                    duration: 0.0,
                }],
                ..WireVitals::default()
            },
        );
        assert_eq!(run.level, 7);
        assert_eq!(run.hp, 50.0);
        assert_eq!(run.abilities[0].level, 3);
        assert!(run.version > before);
        // Crossing the mobility level announces it.
        assert!(run.banner.is_some());
    }

    #[test]
    fn a_pick_the_host_has_not_applied_is_kept() {
        let mut run = view();
        // The guest picked an upgrade the host has not seen yet.
        run.abilities[0].level = 2;
        run.picks = 1;
        run.pending_levels = 0;
        let key = run.abilities[0].def.key.to_string();
        apply_me(
            &mut run,
            WireVitals {
                level: 2,
                pending_levels: 1,
                picks: 0,
                abilities: vec![AbilityView {
                    key,
                    level: 1,
                    cd: 0.2,
                    damage: 0.0,
                    cooldown: 1.0,
                    targets: 0,
                    area: 0.0,
                    duration: 0.0,
                }],
                ..WireVitals::default()
            },
        );
        assert_eq!(run.abilities[0].level, 2, "the optimistic pick stays");
        assert_eq!(run.pending_levels, 0, "no second card dealt for it");
        assert_eq!(
            run.abilities[0].cd, 0.2,
            "the cooldown still follows the host"
        );
    }

    #[test]
    fn unknown_keys_from_a_newer_host_are_skipped() {
        let mut run = view();
        apply_me(
            &mut run,
            WireVitals {
                abilities: vec![AbilityView {
                    key: "no_such_spell".into(),
                    level: 1,
                    cd: 0.0,
                    damage: 0.0,
                    cooldown: 1.0,
                    targets: 0,
                    area: 0.0,
                    duration: 0.0,
                }],
                passives: vec![("nope".into(), 1, 1.0)],
                ..WireVitals::default()
            },
        );
        assert!(run.abilities.is_empty());
        assert!(run.passives.is_empty());
    }
}
