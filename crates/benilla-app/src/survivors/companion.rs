//! The hero's companion: a real 1.12.1 vanity pet ([`data::COMPANIONS`]) bought or earned on the
//! Rewards tab, which runs at its hero's side in every run and fetches what drops. It runs for
//! the pickups within its leash of the hero, first those its instinct ([`data::Instinct`]) likes,
//! and sends each it comes near flying to its hero, as that hero's magnet would; the hero's own
//! magnet does the rest ([`super::enemies::collect_gems`]). Its training (`data::PET_TRAINING`,
//! ranks bought as the perks are) makes the leash longer, the paws quicker and the nose keener
//! ([`data::pet_stats`]).
//!
//! The host's, as everything the run does: it seats every hero's companion from what each
//! brings (`Brings::pet`, the training among `Brings::perks`, `Brings::instinct`), and guests
//! see it as any other unit the host streams.

use bevy::prelude::*;

use super::data::{self, CompanionDef, Instinct, PetStats};
use super::enemies::Gem;
use super::hero::Hero;
use super::units::{self, flat, yaw_toward, UnitSpec};
use super::{HeroState, Run};
use crate::creature_anim::{move_flags, MovementState};

/// A companion, and the hero it runs for.
#[derive(Component, Debug)]
pub(super) struct Companion {
    pub owner: Entity,
    stats: PetStats,
    instinct: Instinct,
    /// The pickup it runs for.
    target: Option<Entity>,
}

/// Where a companion stands by its hero: a little behind, to the right.
const HEEL: Vec3 = Vec3::new(1.2, 0.0, 1.4);
/// Close enough to its heel to stand still.
const AT_HEEL: f32 = 0.6;

/// Spawn `def` at `owner`'s side.
pub(super) fn spawn(
    commands: &mut Commands,
    run: &mut Run,
    def: &CompanionDef,
    owner: Entity,
    stats: PetStats,
    instinct: Instinct,
    pos: Vec3,
) -> Entity {
    let e = units::spawn_unit(
        commands,
        &UnitSpec {
            serial: run.guid(),
            display: def.display,
            scale: 1.0,
            level: 1,
            faction: units::FACTION_FRIENDLY,
            bytes0: 0,
            weapons: &[],
            sheath: 0,
        },
        pos + HEEL,
        0.0,
    );
    commands.entity(e).insert(Companion {
        owner,
        stats,
        instinct,
        target: None,
    });
    e
}

/// How much a companion wants a pickup: lower first.
fn rank(instinct: Instinct, gem: &Gem) -> u8 {
    match instinct {
        Instinct::Nearest => 0,
        Instinct::Gold => u8::from(!gem.is_treasure()),
        Instinct::Wisps => u8::from(!gem.is_xp()),
    }
}

/// The pickup a companion at `at` runs for: a free one within `leash` of its hero at `home`,
/// the most wanted first, then the nearest.
pub(super) fn choose<'a>(
    at: Vec3,
    home: Vec3,
    stats: &PetStats,
    instinct: Instinct,
    gems: impl IntoIterator<Item = (Entity, Vec3, &'a Gem)>,
) -> Option<Entity> {
    gems.into_iter()
        .filter(|(_, p, g)| g.free() && flat(*p - home).length() <= stats.leash)
        .min_by(|a, b| {
            rank(instinct, a.2)
                .cmp(&rank(instinct, b.2))
                .then(flat(a.1 - at).length().total_cmp(&flat(b.1 - at).length()))
        })
        .map(|(e, ..)| e)
}

/// Each companion runs for a pickup, or back to its hero's heel, and sends what it comes near
/// flying to its hero. One whose hero has left goes with it.
#[allow(clippy::type_complexity)]
pub(super) fn fetch(
    mut commands: Commands,
    time: Res<Time>,
    world: benilla_world::world_point::WorldPoint,
    heroes: Query<(&Transform, &HeroState), (With<Hero>, Without<Companion>)>,
    mut pets: Query<
        (Entity, &mut Transform, &mut MovementState, &mut Companion),
        (Without<Hero>, Without<Gem>),
    >,
    mut gems: Query<(Entity, &Transform, &mut Gem), (Without<Companion>, Without<Hero>)>,
) {
    let dt = time.delta_secs();
    for (e, mut tf, mut mv, mut pet) in &mut pets {
        let Ok((htf, h)) = heroes.get(pet.owner) else {
            commands.entity(e).despawn();
            continue;
        };
        let home = htf.translation;
        let heel = home + htf.rotation * HEEL;
        if flat(tf.translation - home).length() > data::PET_TELEPORT {
            tf.translation = heel;
            pet.target = None;
        }
        let at = tf.translation;
        let stats = pet.stats;
        // Its target holds while it is free and within the leash; else the next.
        let fetching = h.alive();
        let keep = pet.target.filter(|t| {
            gems.get(*t).is_ok_and(|(_, g, gem)| {
                gem.free() && flat(g.translation - home).length() <= stats.leash
            })
        });
        pet.target = match keep {
            Some(t) => Some(t),
            None if fetching => choose(
                at,
                home,
                &stats,
                pet.instinct,
                gems.iter().map(|(g, t, gem)| (g, t.translation, gem)),
            ),
            None => None,
        };
        // What it comes near flies to its hero.
        if fetching {
            for (_, g, mut gem) in &mut gems {
                if gem.free() && flat(g.translation - at).length() <= stats.nose {
                    gem.send_to(pet.owner);
                }
            }
        }
        let goal = pet
            .target
            .and_then(|t| gems.get(t).ok())
            .map_or(heel, |(_, g, _)| g.translation);
        let d = flat(goal - at);
        let dist = d.length();
        if dist <= AT_HEEL {
            if mv.speed != 0.0 || mv.flags != 0 {
                mv.speed = 0.0;
                mv.flags = 0;
            }
            continue;
        }
        let step = (stats.speed * dt).min(dist);
        let mut next = at + d / dist * step;
        next.y = world.terrain_height_under(next).unwrap_or(home.y);
        tf.translation = next;
        tf.rotation = Quat::from_rotation_y(yaw_toward(d));
        mv.speed = stats.speed;
        mv.flags = move_flags::FORWARD;
    }
}

#[cfg(test)]
mod tests {
    use super::super::enemies::GemKind;
    use super::*;

    fn e(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn a_companion_runs_for_what_its_instinct_likes_within_its_leash() {
        let stats = data::pet_stats([]);
        let wisp = Gem::loose(GemKind::Xp(5));
        let coin = Gem::loose(GemKind::Gold(3));
        let mut caught = Gem::loose(GemKind::Xp(5));
        caught.send_to(e(9));
        let far = Gem::loose(GemKind::Gold(3));
        let gems = || {
            [
                (e(1), Vec3::new(2.0, 0.0, 0.0), &wisp),
                (e(2), Vec3::new(6.0, 0.0, 0.0), &coin),
                (e(3), Vec3::new(1.0, 0.0, 0.0), &caught),
                (e(4), Vec3::new(stats.leash + 1.0, 0.0, 0.0), &far),
            ]
        };
        let (at, home) = (Vec3::ZERO, Vec3::ZERO);
        // The nearest free one; a caught one and one past the leash never.
        assert_eq!(
            choose(at, home, &stats, Instinct::Nearest, gems()),
            Some(e(1))
        );
        assert_eq!(
            choose(at, home, &stats, Instinct::Wisps, gems()),
            Some(e(1))
        );
        // A gold sniffer runs past the wisp for the coin.
        assert_eq!(choose(at, home, &stats, Instinct::Gold, gems()), Some(e(2)));
        // The leash is measured from the hero, not the companion.
        let home = Vec3::new(30.0, 0.0, 0.0);
        assert_eq!(choose(at, home, &stats, Instinct::Nearest, gems()), None);
    }

    #[test]
    fn training_makes_the_leash_longer_the_paws_quicker_the_nose_keener() {
        let base = data::pet_stats([]);
        assert_eq!(
            (base.leash, base.speed, base.nose),
            (data::PET_LEASH, data::PET_SPEED, data::PET_NOSE)
        );
        let all = data::pet_stats(data::PET_TRAINING.iter().map(|p| (p, 9)));
        assert_eq!(all.leash, data::PET_LEASH + 15.0);
        assert!((all.speed - data::PET_SPEED * 1.6).abs() < 1e-4);
        assert_eq!(all.nose, data::PET_NOSE + 2.0);
        // The perks are not training.
        assert_eq!(data::pet_stats(data::PERKS.iter().map(|p| (p, 3))), base);
    }
}
