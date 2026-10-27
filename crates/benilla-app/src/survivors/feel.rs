//! How a hit feels, on every machine: the struck model brightens for a moment, and the camera
//! shakes when the hero takes a heavy blow, a strike lands critical or a boss comes.
//!
//! Both read [`Struck`], which [`super::units::Fx::number`] writes beside each floating number:
//! the host's own and the ones a guest replays from its stream, so nothing more crosses the wire.
//! The shove a hit gives a foe is the simulation's (`data::knock_impulse`, in
//! `abilities::apply_hits` and `enemies::move_enemies`) and reaches guests as its position.
//!
//! The brighten is the client's own highlight bit (`benilla_world::mesh_tag::HIGHLIGHT_BIT`, the
//! mouseover glow), set after `target::highlight` has had its say, in `Last`. The shake rides on
//! the camera `hero::place_camera` placed, which it overwrites every frame, so it never builds up.
//! The Options page turns the shake off (`Options::shake`).

use bevy::mesh::MeshTag;
use bevy::prelude::*;

use benilla_world::mesh_tag::HIGHLIGHT_BIT;
use benilla_world::view::WorldCamera;

use super::data;
use super::profile::ProfileStore;
use super::{LocalHero, Phase, Run};

/// A number shown over `anchor`: something was struck, or healed.
#[derive(Message, Clone, Copy, Debug)]
pub(super) struct Struck {
    pub anchor: Entity,
    pub amount: f32,
    pub crit: bool,
    pub color: u32,
}

/// The models brightening, and the camera's trauma.
#[derive(Resource, Default)]
pub(super) struct Feel {
    flashes: Vec<(Entity, f32)>,
    /// What brightened last frame, so what has stopped is put out.
    lit: Vec<Entity>,
    /// 0..1; the shake is its square.
    trauma: f32,
    /// What of it crits brought, which they alone never push past [`data::SHAKE_CRIT_MAX`].
    crit_trauma: f32,
    /// The bosses come so far, to shake once at each.
    bosses: u32,
    t: f32,
}

impl Feel {
    fn flash(&mut self, e: Entity) {
        match self.flashes.iter_mut().find(|(f, _)| *f == e) {
            Some((_, t)) => *t = data::FLASH_SECS,
            None => self.flashes.push((e, data::FLASH_SECS)),
        }
    }

    fn shake(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }

    /// A blow to the hero shakes by the share of its health taken; a critical strike shakes a
    /// little, crits alone up to a cap.
    fn take(&mut self, s: &Struck, local: Option<Entity>, max_hp: f32) {
        if s.color == super::units::COLOR_HEAL {
            return;
        }
        self.flash(s.anchor);
        if Some(s.anchor) == local && s.color == super::units::COLOR_HURT {
            self.shake((s.amount / max_hp.max(1.0) * data::SHAKE_PER_HURT).min(0.6));
        } else if s.crit {
            let room = (data::SHAKE_CRIT_MAX - self.crit_trauma).max(0.0);
            let add = data::SHAKE_CRIT.min(room);
            self.crit_trauma += add;
            self.shake(add);
        }
    }

    /// Trauma ebbs, and the crits' share with it.
    fn ebb(&mut self, dt: f32) {
        let before = self.trauma.max(1e-6);
        self.trauma = (self.trauma - data::SHAKE_DECAY * dt).max(0.0);
        self.crit_trauma = (self.crit_trauma * self.trauma / before).min(self.trauma);
    }
}

/// Read the numbers shown this frame: flash what they float over, shake for what they say.
pub(super) fn gather(
    mut struck: MessageReader<Struck>,
    mut feel: ResMut<Feel>,
    run: Res<Run>,
    local: Query<Entity, With<LocalHero>>,
) {
    let local = local.single().ok();
    let max_hp = run.stats().max_hp;
    for s in struck.read() {
        feel.take(s, local, max_hp);
    }
    if !run.begun {
        feel.bosses = 0;
    } else if run.bosses > feel.bosses {
        feel.bosses = run.bosses;
        feel.shake(data::SHAKE_BOSS);
    } else if run.bosses < feel.bosses {
        // A new stage's clock starts over.
        feel.bosses = run.bosses;
    }
}

/// The camera's shake for `trauma` at time `t`: an offset and a small turn, from three slow,
/// unrelated waves, so it judders rather than buzzes.
pub(super) fn shake_at(trauma: f32, t: f32) -> (Vec3, f32, f32) {
    let k = trauma.clamp(0.0, 1.0).powi(2);
    let wave = |f: f32, p: f32| (t * f + p).sin() * 0.6 + (t * f * 2.3 + p * 1.7).sin() * 0.4;
    let offset = Vec3::new(wave(31.0, 0.0), wave(27.0, 1.3), wave(23.0, 2.9)) * data::SHAKE_OFFSET;
    (
        offset * k,
        wave(19.0, 4.1) * data::SHAKE_TURN * k,
        wave(17.0, 5.3) * data::SHAKE_TURN * k,
    )
}

/// Shake the camera `place_camera` just placed, while a run is played.
pub(super) fn shake_camera(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    store: Res<ProfileStore>,
    mut feel: ResMut<Feel>,
    mut cams: Query<&mut Transform, With<WorldCamera>>,
) {
    let dt = time.delta_secs();
    feel.t += dt;
    let playing = matches!(phase.get(), Phase::Playing);
    if !playing || !store.profile.options.shake {
        feel.trauma = 0.0;
        feel.crit_trauma = 0.0;
        return;
    }
    let (offset, yaw, pitch) = shake_at(feel.trauma, feel.t);
    feel.ebb(dt);
    if offset == Vec3::ZERO {
        return;
    }
    for mut cam in &mut cams {
        let turn = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0);
        let along = cam.rotation * offset;
        cam.translation += along;
        cam.rotation *= turn;
    }
}

/// Brighten what was struck, after the client's highlight wrote its own; put out what has stopped.
pub(super) fn flash_models(
    time: Res<Time>,
    mut feel: ResMut<Feel>,
    children: Query<&Children>,
    mut tags: Query<&mut MeshTag>,
) {
    let dt = time.delta_secs();
    let feel = &mut *feel;
    for (_, t) in &mut feel.flashes {
        *t -= dt;
    }
    feel.flashes.retain(|(_, t)| *t > 0.0);
    let now: Vec<Entity> = feel.flashes.iter().map(|(e, _)| *e).collect();
    for &e in &feel.lit {
        if !now.contains(&e) {
            set_bit(e, false, &children, &mut tags);
        }
    }
    for &e in &now {
        set_bit(e, true, &children, &mut tags);
    }
    feel.lit = now;
}

fn set_bit(root: Entity, on: bool, children: &Query<&Children>, tags: &mut Query<&mut MeshTag>) {
    for e in std::iter::once(root).chain(children.iter_descendants(root)) {
        if let Ok(mut tag) = tags.get_mut(e) {
            let bits = if on {
                tag.0 | HIGHLIGHT_BIT
            } else {
                tag.0 & !HIGHLIGHT_BIT
            };
            if tag.0 != bits {
                tag.0 = bits;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::units::{COLOR_HEAL, COLOR_HURT, COLOR_SPELL};

    fn hit(anchor: Entity, amount: f32, crit: bool, color: u32) -> Struck {
        Struck {
            anchor,
            amount,
            crit,
            color,
        }
    }

    #[test]
    fn a_heavy_blow_shakes_more_than_a_light_one() {
        let me = World::new().spawn_empty().id();
        let mut light = Feel::default();
        light.take(&hit(me, 5.0, false, COLOR_HURT), Some(me), 100.0);
        let mut heavy = Feel::default();
        heavy.take(&hit(me, 30.0, false, COLOR_HURT), Some(me), 100.0);
        assert!(heavy.trauma > light.trauma && light.trauma > 0.0);
        assert!(heavy.trauma <= 1.0);
    }

    #[test]
    fn crits_alone_shake_only_so_much() {
        let mut w = World::new();
        let (me, foe) = (w.spawn_empty().id(), w.spawn_empty().id());
        let mut f = Feel::default();
        for _ in 0..100 {
            f.take(&hit(foe, 50.0, true, COLOR_SPELL), Some(me), 100.0);
        }
        assert!((f.trauma - data::SHAKE_CRIT_MAX).abs() < 1e-5);
        // A foe struck flashes; a heal does neither.
        assert_eq!(f.flashes.len(), 1);
        let mut h = Feel::default();
        h.take(&hit(me, 50.0, true, COLOR_HEAL), Some(me), 100.0);
        assert_eq!((h.trauma, h.flashes.len()), (0.0, 0));
    }

    #[test]
    fn trauma_ebbs_away() {
        let mut f = Feel::default();
        f.shake(0.8);
        f.crit_trauma = 0.2;
        f.ebb(0.25);
        assert!(f.trauma < 0.8 && f.crit_trauma <= f.trauma);
        f.ebb(10.0);
        assert_eq!(f.trauma, 0.0);
        assert_eq!(f.crit_trauma, 0.0);
        let (offset, yaw, pitch) = shake_at(0.0, 3.0);
        assert_eq!((offset, yaw, pitch), (Vec3::ZERO, 0.0, 0.0));
        let (offset, ..) = shake_at(1.0, 3.0);
        assert!(offset.length() <= data::SHAKE_OFFSET * 3f32.sqrt() + 1e-4);
    }

    #[test]
    fn heavy_hits_shove_further_and_bosses_least() {
        let light = data::knock_impulse(5.0, 100.0, false, false);
        let heavy = data::knock_impulse(50.0, 100.0, false, false);
        assert!(heavy > light && light >= data::KNOCK_MIN);
        assert!(heavy <= data::KNOCK_MIN + data::KNOCK_SPEED);
        let boss = data::knock_impulse(50.0, 100.0, true, false);
        let elite = data::knock_impulse(50.0, 100.0, false, true);
        assert!(boss < elite && elite < heavy);
        assert_eq!(data::knock_impulse(0.0, 100.0, false, false), 0.0);
    }
}
