//! The hero's target, as in WoW: a click on an enemy selects it (left or right button), a left
//! click on nothing lets it go (the client's `deselectOnClick`), Tab takes the nearest enemy ahead
//! and steps through the next ones, and Escape clears it before it pauses.
//!
//! The ground ring under the target is the client's own: [`HeroTarget`] is mirrored into
//! [`crate::target::Selection`], whose `ring::update_ring` draws it in the target's reaction
//! colour and drops it when the target dies. The hero's single-target spells take the target
//! first while it is within their reach ([`super::abilities`]); in a lobby that holds for the
//! host's own hero, whose spells this machine casts.

use benilla_world::view::WorldCamera;
use bevy::prelude::*;

use super::hero::{CamRig, MouseLook};
use super::{Hostile, LocalHero, Phase};
use crate::net::Guid;

/// The local hero's target.
#[derive(Resource, Default)]
pub(super) struct HeroTarget(pub Option<Entity>);

/// A unit's pickable body, per unit of scale: a column this tall and this wide around its feet.
const PICK_HEIGHT: f32 = 2.2;
const PICK_RADIUS: f32 = 0.9;
/// Tab reaches this far (yd), and counts an enemy as ahead within this cone (cos of the half-angle).
const TAB_RANGE: f32 = 40.0;
const TAB_AHEAD: f32 = 0.35;

/// The unit a ray from the camera hits first: each a column from its feet up, `PICK_HEIGHT` and
/// `PICK_RADIUS` times its scale.
pub(super) fn pick_unit(
    origin: Vec3,
    dir: Vec3,
    units: impl IntoIterator<Item = (Entity, Vec3, f32)>,
) -> Option<Entity> {
    let dir = dir.normalize_or_zero();
    let mut best: Option<(f32, Entity)> = None;
    for (e, feet, scale) in units {
        let (height, radius) = (PICK_HEIGHT * scale, PICK_RADIUS * scale);
        // The column's nearest point to the ray, sampled up its height.
        let mut hit: Option<f32> = None;
        for k in 0..=6 {
            let p = feet + Vec3::Y * (height * k as f32 / 6.0);
            let t = (p - origin).dot(dir);
            if t <= 0.0 {
                continue;
            }
            if (origin + dir * t).distance(p) <= radius {
                hit = Some(hit.map_or(t, |h: f32| h.min(t)));
            }
        }
        if let Some(t) = hit {
            if best.is_none_or(|(bt, _)| t < bt) {
                best = Some((t, e));
            }
        }
    }
    best.map(|(_, e)| e)
}

/// The enemies Tab steps through: within [`TAB_RANGE`], those ahead of `forward` first, each
/// group nearest first.
pub(super) fn tab_order(
    hero: Vec3,
    forward: Vec3,
    units: impl IntoIterator<Item = (Entity, Vec3)>,
) -> Vec<Entity> {
    let mut near: Vec<(bool, f32, Entity)> = units
        .into_iter()
        .filter_map(|(e, p)| {
            let d = super::units::flat(p - hero);
            let dist = d.length();
            (dist <= TAB_RANGE).then(|| {
                let ahead = dist < 0.01 || d.normalize().dot(forward) >= TAB_AHEAD;
                (!ahead, dist, e)
            })
        })
        .collect();
    near.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    near.into_iter().map(|(_, _, e)| e).collect()
}

/// Clicks and Tab pick the target; Escape clears it (the pause waits for the next press).
#[allow(clippy::too_many_arguments)]
pub(super) fn pick_target(
    phase: Res<State<Phase>>,
    look: Res<MouseLook>,
    rig: Res<CamRig>,
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<super::kit::KitFocus>,
    cams: Query<(&Camera, &GlobalTransform), With<WorldCamera>>,
    hero: Query<&Transform, With<LocalHero>>,
    units: Query<(Entity, &Transform), With<Hostile>>,
    mut target: ResMut<HeroTarget>,
) {
    if *phase.get() != Phase::Playing {
        return;
    }
    if let Some((button, at)) = look.clicked {
        let picked = cams.single().ok().and_then(|(cam, gt)| {
            let ray = cam.viewport_to_world(gt, at).ok()?;
            pick_unit(
                ray.origin,
                *ray.direction,
                units
                    .iter()
                    .map(|(e, tf)| (e, tf.translation, tf.scale.x.max(0.1))),
            )
        });
        match (picked, button) {
            (Some(e), _) => target.0 = Some(e),
            (None, MouseButton::Left) => target.0 = None,
            _ => {}
        }
    }
    if !focus.typing() && keys.just_pressed(KeyCode::Tab) {
        let Ok(htf) = hero.single() else { return };
        let forward = (htf.rotation * Vec3::NEG_Z)
            .with_y(0.0)
            .normalize_or(rig.forward());
        let order = tab_order(
            htf.translation,
            forward,
            units.iter().map(|(e, tf)| (e, tf.translation)),
        );
        let next = match target.0.and_then(|t| order.iter().position(|&e| e == t)) {
            Some(i) => order.get(i + 1).or(order.first()).copied(),
            None => order.first().copied(),
        };
        if next.is_some() {
            target.0 = next;
        }
    }
}

/// The target holds while its unit stands; it is mirrored into the client's selection, whose
/// ring draws under it, and a selection the client drops (the unit died) drops it here too.
pub(super) fn keep_target(
    phase: Res<State<Phase>>,
    mut target: ResMut<HeroTarget>,
    units: Query<(Option<&Guid>, Option<&super::enemies::Enemy>), With<Hostile>>,
    selection: Option<ResMut<crate::target::Selection>>,
    mut mirrored: Local<Option<Entity>>,
) {
    let alive = |e: Entity| {
        units
            .get(e)
            .is_ok_and(|(_, enemy)| enemy.is_none_or(|en| !en.dead))
    };
    if !matches!(phase.get(), Phase::Playing | Phase::LevelUp | Phase::Paused)
        || target.0.is_some_and(|e| !alive(e))
    {
        if target.0.is_some() {
            target.0 = None;
        }
    }
    let Some(mut selection) = selection else {
        *mirrored = target.0;
        return;
    };
    // The client let go of what we gave it: the ring's death clear, or a teardown.
    if target.0.is_some() && target.0 == *mirrored && selection.target != target.0 {
        target.0 = None;
    }
    if *mirrored != target.0 {
        *mirrored = target.0;
        let guid = target
            .0
            .and_then(|e| units.get(e).ok())
            .and_then(|(g, _)| g.map(|g| g.0));
        selection.target = target.0.filter(|_| guid.is_some());
        selection.guid = guid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(n: u32) -> Entity {
        Entity::from_raw_u32(n).unwrap()
    }

    #[test]
    fn a_click_picks_the_nearest_unit_the_ray_meets() {
        let origin = Vec3::new(0.0, 5.0, 10.0);
        // Two in a line along the ray's path, one off to the side.
        let units = [
            (e(1), Vec3::new(0.0, 0.0, -10.0), 1.0),
            (e(2), Vec3::new(0.0, 0.0, 0.0), 1.0),
            (e(3), Vec3::new(8.0, 0.0, 0.0), 1.0),
        ];
        let toward = |p: Vec3| p + Vec3::Y - origin;
        assert_eq!(pick_unit(origin, toward(units[1].1), units), Some(e(2)));
        assert_eq!(pick_unit(origin, toward(units[2].1), units), Some(e(3)));
        // Past every unit: nothing.
        assert_eq!(pick_unit(origin, Vec3::new(1.0, 0.2, 0.0), units), None);
        // A giant's column is wider.
        let giant = [(e(4), Vec3::new(2.0, 0.0, 0.0), 3.0)];
        assert_eq!(pick_unit(origin, toward(Vec3::ZERO), giant), Some(e(4)));
    }

    #[test]
    fn tab_takes_what_is_ahead_first_nearest_first() {
        let hero = Vec3::ZERO;
        let forward = Vec3::NEG_Z;
        let order = tab_order(
            hero,
            forward,
            [
                (e(1), Vec3::new(0.0, 0.0, 3.0)),   // behind, near
                (e(2), Vec3::new(0.0, 0.0, -12.0)), // ahead, far
                (e(3), Vec3::new(1.0, 0.0, -5.0)),  // ahead, near
                (e(4), Vec3::new(0.0, 0.0, -80.0)), // out of reach
            ],
        );
        assert_eq!(order, [e(3), e(2), e(1)]);
    }
}
