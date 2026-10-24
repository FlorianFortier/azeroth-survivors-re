//! A hero's numbers, from every source at once. Every source speaks [`Bonus`]: the class's head
//! start, the run's passives, the bonuses the hero brings into the run ([`super::Vitals::meta`]:
//! the collection, talents, racials and gear, as each lands), and the effects running on it now
//! ([`Timed`], fired by a [`ProcDef`]). [`Stats::from_bonuses`] is the one place they become
//! numbers: `(base + Σflat) × (1 + Σpercent)` per stat.

use super::data::{Amount, Bonus, ProcDef, Stat, MAX_DAMAGE_REDUCTION};

/// The hero's derived numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Stats {
    pub might: f32,
    pub max_hp: f32,
    pub speed: f32,
    pub regen: f32,
    pub haste: f32,
    pub area: f32,
    pub magnet: f32,
    pub growth: f32,
    pub armor: f32,
    pub crit: f32,
    /// The fraction of each hit ignored, already capped.
    pub damage_reduction: f32,
    pub gold: f32,
}

impl Stats {
    /// Every stat at its base, no bonus.
    pub fn base() -> Self {
        Self::from_bonuses(std::iter::empty())
    }

    pub fn from_bonuses<'a>(bonuses: impl IntoIterator<Item = &'a Bonus>) -> Self {
        let n = Stat::ALL.len();
        let mut flat = vec![0.0f32; n];
        let mut pct = vec![0.0f32; n];
        for b in bonuses {
            let i = index(b.stat);
            match b.amount {
                Amount::Flat(v) => flat[i] += v,
                Amount::Percent(v) => pct[i] += v,
            }
        }
        let at = |s: Stat| {
            let i = index(s);
            (s.base() + flat[i]) * (1.0 + pct[i])
        };
        Self {
            might: at(Stat::Might),
            max_hp: at(Stat::MaxHp).max(1.0),
            speed: at(Stat::Speed).max(0.0),
            regen: at(Stat::Regen),
            haste: at(Stat::Haste),
            area: at(Stat::Area).max(0.0),
            magnet: at(Stat::Magnet).max(0.0),
            growth: at(Stat::Growth).max(0.0),
            armor: at(Stat::Armor),
            crit: at(Stat::Crit).clamp(0.0, 1.0),
            damage_reduction: at(Stat::DamageReduction).clamp(0.0, MAX_DAMAGE_REDUCTION),
            gold: at(Stat::Gold).max(0.0),
        }
    }

    /// The number for one stat.
    pub fn get(&self, stat: Stat) -> f32 {
        match stat {
            Stat::Might => self.might,
            Stat::MaxHp => self.max_hp,
            Stat::Speed => self.speed,
            Stat::Regen => self.regen,
            Stat::Haste => self.haste,
            Stat::Area => self.area,
            Stat::Magnet => self.magnet,
            Stat::Growth => self.growth,
            Stat::Armor => self.armor,
            Stat::Crit => self.crit,
            Stat::DamageReduction => self.damage_reduction,
            Stat::Gold => self.gold,
        }
    }
}

fn index(stat: Stat) -> usize {
    Stat::ALL
        .iter()
        .position(|s| *s == stat)
        .expect("every stat is in Stat::ALL")
}

/// An effect running on a hero, granting its bonuses until it runs out.
#[derive(Clone, Debug)]
pub(crate) struct Timed {
    pub key: &'static str,
    /// Its visual on the hero, 0 for none.
    pub spell: u32,
    pub bonuses: &'static [Bonus],
    pub left: f32,
}

/// A [`ProcDef`] on a hero, counting down to its next firing.
#[derive(Clone)]
pub(crate) struct Proc {
    pub def: &'static ProcDef,
    pub next: f32,
}

impl Proc {
    /// Ready to fire a first time a full interval into the run.
    pub fn new(def: &'static ProcDef) -> Self {
        Self {
            def,
            next: def.every,
        }
    }
}

/// Advance the procs and the running effects by `dt`; returns the procs that fired, for their
/// visuals. A proc firing again while its effect still runs refreshes it rather than stacking.
pub(crate) fn tick_effects(
    procs: &mut [Proc],
    timed: &mut Vec<Timed>,
    dt: f32,
) -> Vec<&'static ProcDef> {
    for t in timed.iter_mut() {
        t.left -= dt;
    }
    timed.retain(|t| t.left > 0.0);
    let mut fired = Vec::new();
    for p in procs.iter_mut() {
        p.next -= dt;
        if p.next > 0.0 {
            continue;
        }
        p.next += p.def.every.max(0.1);
        let def = p.def;
        match timed.iter_mut().find(|t| t.key == def.key) {
            Some(t) => t.left = t.left.max(def.duration),
            None => timed.push(Timed {
                key: def.key,
                spell: def.spell,
                bonuses: def.bonuses,
                left: def.duration,
            }),
        }
        fired.push(def);
    }
    fired
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::data::{self, Bonus, Stat};

    #[test]
    fn no_bonus_is_the_base() {
        let s = Stats::base();
        for stat in Stat::ALL {
            assert_eq!(s.get(stat), stat.base(), "{stat:?}");
        }
    }

    #[test]
    fn flat_then_percent() {
        let s = Stats::from_bonuses(&[
            Bonus::flat(Stat::MaxHp, 30.0),
            Bonus::pct(Stat::MaxHp, 0.5),
            Bonus::pct(Stat::MaxHp, 0.5),
            Bonus::flat(Stat::Crit, 0.01),
        ]);
        assert_eq!(s.max_hp, (120.0 + 30.0) * 2.0);
        assert!((s.crit - 0.06).abs() < 1e-6);
    }

    #[test]
    fn caps_hold() {
        let s = Stats::from_bonuses(&[
            Bonus::flat(Stat::DamageReduction, 2.0),
            Bonus::flat(Stat::Crit, 3.0),
            Bonus::pct(Stat::MaxHp, -5.0),
            Bonus::pct(Stat::Speed, -5.0),
        ]);
        assert_eq!(s.damage_reduction, data::MAX_DAMAGE_REDUCTION);
        assert_eq!(s.crit, 1.0);
        assert_eq!(s.max_hp, 1.0);
        assert_eq!(s.speed, 0.0);
    }

    /// The model must not move any number the game already had: the class head starts and
    /// every passive at every rank give what the old hand-written sums gave.
    #[test]
    fn the_old_numbers_are_kept() {
        for class in data::CLASSES {
            for p in data::PASSIVES {
                for level in 1..=data::MAX_PASSIVE_LEVEL {
                    let mut bonuses: Vec<Bonus> = data::class_bonuses(class.id).to_vec();
                    bonuses.push(p.bonus(level));
                    let new = Stats::from_bonuses(&bonuses);

                    // The sums as the game wrote them before.
                    let (mut hp, mut speed) = (120.0f32, 7.0f32);
                    match class.id {
                        1 | 2 => hp += 40.0,
                        7 | 11 | 3 | 4 => hp += 15.0,
                        _ => speed += 0.5,
                    }
                    let mut old = Stats {
                        max_hp: hp,
                        speed,
                        ..Stats::base()
                    };
                    let v = p.per_level * f32::from(level);
                    match p.stat {
                        Stat::Might => old.might += v,
                        Stat::MaxHp => old.max_hp *= 1.0 + v,
                        Stat::Speed => old.speed *= 1.0 + v,
                        Stat::Regen => old.regen += v,
                        Stat::Haste => old.haste += v,
                        Stat::Area => old.area += v,
                        Stat::Magnet => old.magnet *= 1.0 + v,
                        Stat::Growth => old.growth += v,
                        Stat::Armor => old.armor += v,
                        Stat::Crit => old.crit += v,
                        Stat::DamageReduction | Stat::Gold => unreachable!("no such passive"),
                    }
                    for stat in Stat::ALL {
                        assert!(
                            (new.get(stat) - old.get(stat)).abs() < 1e-4,
                            "{} {} rank {level}: {stat:?} {} vs {}",
                            class.name,
                            p.key,
                            new.get(stat),
                            old.get(stat)
                        );
                    }
                }
            }
        }
    }

    static FRENZY: ProcDef = ProcDef {
        key: "frenzy",
        spell: 20554,
        every: 10.0,
        duration: 4.0,
        bonuses: &[Bonus::flat(Stat::Haste, 0.3)],
    };

    #[test]
    fn a_proc_fires_on_its_interval_and_its_effect_runs_out() {
        let mut procs = vec![Proc::new(&FRENZY)];
        let mut timed = Vec::new();
        // Nothing for the first interval.
        assert!(tick_effects(&mut procs, &mut timed, 9.9).is_empty());
        assert!(timed.is_empty());
        // Fires.
        let fired = tick_effects(&mut procs, &mut timed, 0.2);
        assert_eq!(fired.len(), 1);
        assert_eq!(timed.len(), 1);
        let s = Stats::from_bonuses(timed.iter().flat_map(|t| t.bonuses));
        assert!((s.haste - 0.3).abs() < 1e-6);
        // Runs out after its duration, and fires again on the next interval.
        tick_effects(&mut procs, &mut timed, 4.0);
        assert!(timed.is_empty());
        let fired = tick_effects(&mut procs, &mut timed, 5.9);
        assert_eq!(fired.len(), 1);
    }

    #[test]
    fn a_refired_effect_refreshes_rather_than_stacks() {
        static LONG: ProcDef = ProcDef {
            key: "long",
            spell: 0,
            every: 1.0,
            duration: 5.0,
            bonuses: &[Bonus::flat(Stat::Might, 0.5)],
        };
        let mut procs = vec![Proc::new(&LONG)];
        let mut timed = Vec::new();
        for _ in 0..4 {
            tick_effects(&mut procs, &mut timed, 1.0);
        }
        assert_eq!(timed.len(), 1);
        let s = Stats::from_bonuses(timed.iter().flat_map(|t| t.bonuses));
        assert!((s.might - 1.5).abs() < 1e-6);
    }
}
