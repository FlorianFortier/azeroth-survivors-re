//! The class's mobility spell: learned at [`LEARN_LEVEL`], cast only on `F`, never by itself. Each
//! is the class's own signature escape: a mage blinks, a rogue sprints, a warrior charges.

use bevy::prelude::*;

use super::enemies::Enemy;
use super::hero::{self, Hero};
use super::units::{flat, Fx};
use super::{controls_active, HeroState, Hostile, LocalHero, Phase, Run, Session, Vitals};
use crate::net::{Guid, ObjectStore};
use benilla_world::world_point::WorldPoint;
use survivors_net::game::MobilityEffect;

pub(super) const LEARN_LEVEL: u32 = 5;
pub(super) const KEY: KeyCode = KeyCode::KeyF;

#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Move {
    /// Jump this many yards along the way the hero faces.
    Blink(f32),
    /// Rush at the nearest enemy within this many yards, stunning it; else lunge that far.
    Charge(f32),
    /// Run faster by this factor for `secs`.
    Haste(f32),
    /// Haste, and enemies lose the hero (they stop chasing) for `secs`.
    Vanish(f32),
    /// Haste, and every enemy within this many yards flees in fear.
    Scatter(f32),
}

pub(super) struct Mobility {
    pub spell: u32,
    pub name: &'static str,
    pub kind: Move,
    pub cooldown: f32,
    pub secs: f32,
    pub desc: &'static str,
    /// The 1.12.1 spell whose visual it plays, when `spell` is Legion's; 0 plays `spell`'s.
    pub visual: u32,
}

impl Mobility {
    /// The spell whose visual it plays.
    pub(super) fn visual(&self) -> u32 {
        if self.visual != 0 {
            self.visual
        } else {
            self.spell
        }
    }
}

/// One per class, indexed by class id.
pub(super) fn for_class(class: u8) -> Option<&'static Mobility> {
    MOBILITY.iter().find(|(c, _)| *c == class).map(|(_, m)| m)
}

static MOBILITY: &[(u8, Mobility)] = &[
    (
        1,
        Mobility {
            spell: 100,
            name: "Charge",
            kind: Move::Charge(25.0),
            cooldown: 8.0,
            secs: 1.5,
            desc: "Charge the nearest enemy within 25 yards, stunning it.",
            visual: 0,
        },
    ),
    (
        2,
        Mobility {
            spell: 1044,
            name: "Blessing of Freedom",
            kind: Move::Haste(1.6),
            cooldown: 15.0,
            secs: 5.0,
            desc: "Nothing can hold you: run 60% faster for 5 sec.",
            visual: 0,
        },
    ),
    (
        3,
        Mobility {
            spell: 5118,
            name: "Aspect of the Cheetah",
            kind: Move::Haste(1.7),
            cooldown: 14.0,
            secs: 5.0,
            desc: "Run 70% faster for 5 sec.",
            visual: 0,
        },
    ),
    (
        4,
        Mobility {
            spell: 2983,
            name: "Sprint",
            kind: Move::Haste(1.8),
            cooldown: 15.0,
            secs: 4.0,
            desc: "Run 80% faster for 4 sec.",
            visual: 0,
        },
    ),
    (
        5,
        Mobility {
            spell: 586,
            name: "Fade",
            kind: Move::Vanish(1.4),
            cooldown: 18.0,
            secs: 4.0,
            desc: "Fade from the horde's sight: enemies lose you and you run 40% faster for 4 sec.",
            visual: 0,
        },
    ),
    (
        7,
        Mobility {
            spell: 2645,
            name: "Ghost Wolf",
            kind: Move::Haste(1.6),
            cooldown: 15.0,
            secs: 6.0,
            desc: "Take the form of a ghost wolf, running 60% faster for 6 sec.",
            visual: 0,
        },
    ),
    (
        8,
        Mobility {
            spell: 1953,
            name: "Blink",
            kind: Move::Blink(18.0),
            cooldown: 10.0,
            secs: 0.0,
            desc: "Teleport 18 yards forward.",
            visual: 0,
        },
    ),
    (
        9,
        Mobility {
            spell: 5484,
            name: "Howl of Terror",
            kind: Move::Scatter(12.0),
            cooldown: 20.0,
            secs: 4.0,
            desc: "Howl, sending every enemy within 12 yards fleeing in terror for 4 sec.",
            visual: 0,
        },
    ),
    (
        6,
        Mobility {
            spell: 48265,
            name: "Death's Advance",
            kind: Move::Haste(1.7),
            cooldown: 15.0,
            secs: 5.0,
            desc: "Run 70% faster for 5 sec.",
            visual: 2983,
        },
    ),
    (
        11,
        Mobility {
            spell: 1850,
            name: "Dash",
            kind: Move::Haste(1.8),
            cooldown: 15.0,
            secs: 4.0,
            desc: "Run 80% faster for 4 sec.",
            visual: 0,
        },
    ),
];

/// The mobility's run state, on [`Vitals`]: cooldown left, and the haste or vanish still running.
#[derive(Default, Clone, Copy)]
pub(super) struct MobilityState {
    pub cd: f32,
    pub haste: f32,
    pub haste_t: f32,
    pub vanish_t: f32,
}

impl Vitals {
    /// The class's mobility once the hero is level enough.
    pub(super) fn mobility(&self) -> Option<&'static Mobility> {
        (self.level >= LEARN_LEVEL)
            .then(|| for_class(self.class.id))
            .flatten()
    }

    /// The run speed's multiplier from a haste still running.
    pub(super) fn haste_mult(&self) -> f32 {
        if self.mobile.haste_t > 0.0 {
            self.mobile.haste
        } else {
            1.0
        }
    }
}

/// Mobility casts waiting for the simulation: `(seat, spell, what it does to the horde)`. The
/// local hero's land here directly; a guest's arrive over the network.
#[derive(Resource, Default)]
pub(super) struct MobilityQueue(pub Vec<(u8, u32, MobilityEffect)>);

/// `F`: cast the local hero's mobility if it is learned and ready; ticks the cooldown and the
/// running effect. The hero moves here, on the machine that steers it; the cast's visual and its
/// effect on the horde go through [`MobilityQueue`] to whoever plays the run.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn cast_mobility(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    phase: Res<State<Phase>>,
    session: Res<Session>,
    mut run: ResMut<Run>,
    world: WorldPoint,
    mut hero: Query<&mut Transform, With<LocalHero>>,
    foes: Query<(&Transform, &Guid, &ObjectStore), (With<Hostile>, Without<LocalHero>)>,
    selection: Res<super::Selection>,
    aim: Res<hero::AimPoint>,
    mut queue: ResMut<MobilityQueue>,
) {
    let dt = time.delta_secs();
    let m = &mut run.mobile;
    m.cd = (m.cd - dt).max(0.0);
    m.haste_t = (m.haste_t - dt).max(0.0);
    m.vanish_t = (m.vanish_t - dt).max(0.0);
    if !controls_active(*phase.get(), &session)
        || !run.begun
        || run.dead_t.is_some()
        || !keys.just_pressed(KEY)
    {
        return;
    }
    let Some(def) = run.mobility() else { return };
    if run.mobile.cd > 0.0 {
        return;
    }
    let Ok(mut tf) = hero.single_mut() else {
        return;
    };
    let pos = tf.translation;
    let facing = tf.rotation * Vec3::NEG_Z;
    let facing = flat(facing).normalize_or(Vec3::NEG_Z);
    // Aimed at the cursor where there is one, else straight ahead.
    let cursor = aim.0.filter(|a| flat(*a - pos).length() > 0.5);
    let toward = cursor.map_or(facing, |a| flat(a - pos).normalize());
    // Keep any jump inside the battleground's leash.
    let map = selection.stage_def();
    let center = hero::arena(map);
    let leash = |p: Vec3| {
        let off = flat(p - center);
        if off.length() > map.leash {
            center + off.normalize() * map.leash
        } else {
            p
        }
    };
    let ground = |p: Vec3| world.terrain_height_under(p).unwrap_or(p.y);
    let mut effect = MobilityEffect::None;
    match def.kind {
        Move::Blink(yd) => {
            // To the cursor, or as far as the spell reaches toward it.
            let far = cursor.map_or(yd, |a| flat(a - pos).length().min(yd));
            tf.rotation = Quat::from_rotation_y(super::units::yaw_toward(toward));
            let mut to = leash(pos + toward * far);
            to.y = ground(to);
            tf.translation = to;
        }
        Move::Charge(yd) => {
            // The enemy in reach nearest the cursor (or the hero, with none).
            let mark = cursor.unwrap_or(pos);
            let target = foes
                .iter()
                .filter(|(t, _, store)| {
                    !super::units::is_dead(store) && hero::reach(pos, t.translation) <= yd
                })
                .map(|(t, g, _)| (g.0, t.translation, hero::reach(mark, t.translation)))
                .min_by(|a, b| a.2.total_cmp(&b.2));
            let to = match target {
                Some((guid, at, _)) => {
                    effect = MobilityEffect::Stun {
                        target: guid,
                        secs: def.secs,
                    };
                    let dir = flat(at - pos).normalize_or(facing);
                    tf.rotation = Quat::from_rotation_y(super::units::yaw_toward(dir));
                    at - dir * 1.5
                }
                None => {
                    tf.rotation = Quat::from_rotation_y(super::units::yaw_toward(toward));
                    pos + toward * yd * 0.5
                }
            };
            let mut to = leash(to);
            to.y = ground(to);
            tf.translation = to;
        }
        Move::Haste(x) => {
            run.mobile.haste = x;
            run.mobile.haste_t = def.secs;
        }
        Move::Vanish(x) => {
            run.mobile.haste = x;
            run.mobile.haste_t = def.secs;
            run.mobile.vanish_t = def.secs;
        }
        Move::Scatter(r) => {
            effect = MobilityEffect::Fear {
                at: pos.to_array(),
                radius: r,
                secs: def.secs,
            };
        }
    }
    run.mobile.cd = def.cooldown;
    queue.0.push((session.slot, def.visual(), effect));
}

/// The run plays every mobility cast: its visual on the caster, and its hold on the horde.
pub(super) fn apply_mobility(
    mut queue: ResMut<MobilityQueue>,
    heroes: Query<(Entity, &HeroState), With<Hero>>,
    mut enemies: Query<(Entity, &Guid, &Transform, &mut Enemy)>,
    mut fx: Fx,
) {
    for (slot, spell, effect) in queue.0.drain(..) {
        let Some((me, _)) = heroes.iter().find(|(_, h)| h.slot == slot) else {
            continue;
        };
        match effect {
            MobilityEffect::None => fx.cast(me, spell, &[me], None),
            MobilityEffect::Stun { target, secs } => {
                let hit = enemies.iter_mut().find(|(_, g, ..)| g.0 == target);
                match hit {
                    Some((e, _, _, mut en)) => {
                        if !en.dead {
                            en.stun_t = en.stun_t.max(secs.clamp(0.0, 5.0));
                        }
                        fx.cast(me, spell, &[e], None);
                    }
                    None => fx.cast(me, spell, &[me], None),
                }
            }
            MobilityEffect::Fear { at, radius, secs } => {
                fx.cast(me, spell, &[me], None);
                let at = Vec3::from_array(at);
                let radius = radius.clamp(0.0, 20.0);
                for (_, _, t, mut en) in &mut enemies {
                    if !en.dead && !en.boss && hero::reach(at, t.translation) <= radius {
                        en.fear_t = en.fear_t.max(secs.clamp(0.0, 6.0));
                    }
                }
            }
        }
    }
}
