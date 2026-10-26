//! Smooth motion for what another machine moves: snapshot interpolation. Each position arrives
//! with the sender's clock; a [`Track`] keeps the last few, and the unit is drawn [`DELAY`] behind
//! the sender's now, between two positions it really held. A [`Clock`] maps the sender's clock to
//! ours by the quickest arrival, so a late packet does not drag the picture with it.
//!
//! Without it, a copy chased its latest position: it sped up and slowed down at every one of the
//! twenty a second, and two clocks at twenty a second (a guest's sends, the host's ticks) beat
//! against each other, a tick carrying two steps or none.

use std::collections::VecDeque;

use bevy::prelude::*;

/// How far behind the sender's now a copy is drawn: two of the host's ticks, so one late packet
/// still leaves a position ahead to move toward.
pub(super) const DELAY: f32 = 0.1;
/// A step farther than this between two positions is a jump (a blink, a charge, a new zone).
pub(super) const SNAP_YD: f32 = 10.0;
/// How many positions a track keeps.
const KEEP: usize = 12;
/// How fast the clock's offset is let drift later, in seconds per second, so a path that grows
/// slower is followed; an earlier arrival pulls it in at once.
const RELAX: f32 = 0.02;
/// A sender clock that runs back this far restarted (a new run): the offset is measured again.
const RESTART: f32 = 1.0;

/// One position the sender held, at its own time `t`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sample {
    t: f32,
    pos: Vec3,
    yaw: f32,
}

/// The last positions of one unit, on the sender's clock.
#[derive(Component, Default, Debug)]
pub(super) struct Track {
    samples: VecDeque<Sample>,
}

impl Track {
    /// A position at the sender's time `t`. One older than the last is dropped; a jump clears the
    /// track; after a pause longer than `gap` (a unit stood still and sent nothing), the last
    /// position is held until `gap` before this one, so the walk starts when it really did.
    pub fn push(&mut self, t: f32, pos: Vec3, yaw: f32, gap: f32) {
        if let Some(last) = self.samples.back().copied() {
            if t <= last.t {
                return;
            }
            if last.pos.distance(pos) > SNAP_YD {
                self.samples.clear();
            } else if t - last.t > gap * 1.5 {
                self.samples.push_back(Sample { t: t - gap, ..last });
            }
        }
        self.samples.push_back(Sample { t, pos, yaw });
        while self.samples.len() > KEEP {
            self.samples.pop_front();
        }
    }

    /// Where the unit was at the sender's time `t`: between the two positions around it, else
    /// the nearest end (the newest while nothing newer has come).
    pub fn at(&self, t: f32) -> Option<(Vec3, f32)> {
        let first = self.samples.front()?;
        if t <= first.t {
            return Some((first.pos, first.yaw));
        }
        for (a, b) in self.samples.iter().zip(self.samples.iter().skip(1)) {
            if t <= b.t {
                let k = ((t - a.t) / (b.t - a.t)).clamp(0.0, 1.0);
                return Some((a.pos.lerp(b.pos, k), a.yaw + angle_to(a.yaw, b.yaw) * k));
            }
        }
        let last = self.samples.back()?;
        Some((last.pos, last.yaw))
    }
}

/// `to - from` the short way round, in `-π..=π`.
fn angle_to(from: f32, to: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (to - from + PI).rem_euclid(TAU) - PI
}

/// The sender's clock against ours: `ours - theirs`, from the quickest arrival.
#[derive(Default, Debug, Clone, Copy)]
pub(super) struct Clock {
    offset: Option<f32>,
    last_remote: f32,
    last_local: f32,
}

impl Clock {
    /// A message stamped `remote` arrived at our `local` time.
    pub fn sync(&mut self, local: f32, remote: f32) {
        let seen = local - remote;
        self.offset = Some(match self.offset {
            Some(_) if remote < self.last_remote - RESTART => seen,
            Some(off) => {
                let relaxed = off + (local - self.last_local).max(0.0) * RELAX;
                relaxed.min(seen)
            }
            None => seen,
        });
        self.last_remote = remote;
        self.last_local = local;
    }

    /// The sender's time to draw at, [`DELAY`] behind its now; `None` before any message.
    pub fn draw_at(&self, local: f32) -> Option<f32> {
        self.offset.map(|off| local - off - DELAY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TICK: f32 = 0.05;

    #[test]
    fn a_track_draws_between_the_positions_it_was_sent() {
        let mut t = Track::default();
        t.push(1.0, Vec3::ZERO, 0.0, TICK);
        t.push(1.05, Vec3::X, 0.0, TICK);
        t.push(1.1, Vec3::X * 2.0, 0.0, TICK);
        let (p, _) = t.at(1.025).unwrap();
        assert!(p.distance(Vec3::X * 0.5) < 1e-5);
        let (p, _) = t.at(1.075).unwrap();
        assert!(p.distance(Vec3::X * 1.5) < 1e-5);
        // Past the newest: held there, never guessed beyond it.
        assert_eq!(t.at(5.0).unwrap().0, Vec3::X * 2.0);
        // Before the oldest: the oldest.
        assert_eq!(t.at(0.0).unwrap().0, Vec3::ZERO);
        // An old one arriving late is dropped.
        t.push(1.07, Vec3::Y * 9.0, 0.0, TICK);
        assert!(t.at(1.075).unwrap().0.distance(Vec3::X * 1.5) < 1e-5);
    }

    #[test]
    fn a_unit_that_stood_still_starts_walking_when_it_did() {
        let mut t = Track::default();
        t.push(1.0, Vec3::ZERO, 0.0, TICK);
        // Nothing for a while (it stood still), then a step.
        t.push(5.0, Vec3::X, 0.0, TICK);
        // Still standing until one tick before the step.
        assert_eq!(t.at(4.9).unwrap().0, Vec3::ZERO);
        assert!(t.at(4.975).unwrap().0.distance(Vec3::X * 0.5) < 1e-5);
    }

    #[test]
    fn a_jump_is_drawn_at_once() {
        let mut t = Track::default();
        t.push(1.0, Vec3::ZERO, 0.0, TICK);
        t.push(1.05, Vec3::X * 50.0, 0.0, TICK);
        assert_eq!(t.at(1.01).unwrap().0, Vec3::X * 50.0);
    }

    #[test]
    fn the_facing_turns_the_short_way() {
        let mut t = Track::default();
        t.push(1.0, Vec3::ZERO, 3.0, TICK);
        t.push(1.05, Vec3::ZERO, -3.0, TICK);
        let (_, yaw) = t.at(1.025).unwrap();
        assert!((yaw - (3.0 + (std::f32::consts::TAU - 6.0) / 2.0)).abs() < 1e-4);
    }

    #[test]
    fn the_clock_follows_the_quickest_arrival() {
        let mut c = Clock::default();
        assert_eq!(c.draw_at(10.0), None);
        // Sent at 1.0, 1.05, 1.1; the second is held up and arrives with the third.
        c.sync(11.0, 1.0);
        c.sync(11.2, 1.05);
        c.sync(11.2, 1.1);
        // The late ones do not move the offset: at 11.2 the sender's now is 1.2, drawn DELAY
        // behind it.
        let at = c.draw_at(11.2).unwrap();
        assert!((at - (1.2 - DELAY)).abs() < 0.01, "{at}");
        // A new run: the sender's clock starts over, and the offset follows at once.
        c.sync(20.0, 0.0);
        assert!((c.draw_at(20.0).unwrap() - (0.0 - DELAY)).abs() < 1e-4);
    }
}
