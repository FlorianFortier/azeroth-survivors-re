//! Between the run's heroes and the local player's view of them: seating the heroes when a run
//! begins, landing level-up cards on the hero they belong to, and mirroring the local hero into
//! [`Run`] for the HUD.
//!
//! The view is optimistic: a card picked here shows at once, and the authoritative hero catches
//! up the same frame (solo, host) or a round trip later (guest, see `net::client`).

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use survivors_net::game::Choice as WireChoice;

use super::data;
use super::hero::{self, Hero};
use super::hud::Choice;
use super::{HeroState, LocalHero, Mode, Owned, Phase, Run, Selection, Session, Vitals};

/// Cards the local player picked, for the run to apply (solo, host) or to send (guest).
#[derive(Resource, Default)]
pub(super) struct LocalPicks(pub Vec<WireChoice>);

/// A card as both ends of the wire name it: by key, not by index.
pub(super) fn to_wire(choice: Choice, v: &Vitals) -> WireChoice {
    match choice {
        Choice::NewAbility(def) => WireChoice::NewAbility(def.key.to_string()),
        Choice::Upgrade(k) => WireChoice::Upgrade(
            v.abilities
                .get(k)
                .map_or_else(String::new, |o| o.def.key.to_string()),
        ),
        Choice::NewPassive(p) => WireChoice::NewPassive(p.key.to_string()),
        Choice::UpPassive(k) => WireChoice::UpPassive(
            v.passives
                .get(k)
                .map_or_else(String::new, |(p, _)| p.key.to_string()),
        ),
        Choice::Heal => WireChoice::Heal,
    }
}

/// Land a card on a hero, if it is one the hero could have been dealt: a banked level, an ability
/// of its class it lacks with a slot free, a rank below the cap. Anything else is refused, so a
/// guest cannot pick what the rules would never deal.
pub(super) fn apply_choice(v: &mut Vitals, choice: &WireChoice) -> bool {
    if v.pending_levels == 0 {
        return false;
    }
    let ok = match choice {
        WireChoice::NewAbility(key) => {
            let fits = v.class.pool.contains(&key.as_str())
                && v.abilities.len() < data::MAX_ABILITIES
                && !v.abilities.iter().any(|o| o.def.key == key);
            if fits {
                v.abilities.push(Owned {
                    def: data::ability(key),
                    level: 1,
                    cd: 0.3,
                });
            }
            fits
        }
        WireChoice::Upgrade(key) => match v.abilities.iter_mut().find(|o| o.def.key == key) {
            Some(o) if o.level < data::MAX_ABILITY_LEVEL => {
                o.level += 1;
                true
            }
            _ => false,
        },
        WireChoice::NewPassive(key) => {
            let def = data::PASSIVES.iter().find(|p| p.key == key);
            match def {
                Some(p)
                    if v.passives.len() < data::MAX_PASSIVES
                        && !v.passives.iter().any(|(q, _)| q.key == p.key) =>
                {
                    v.passives.push((p, 1));
                    true
                }
                _ => false,
            }
        }
        WireChoice::UpPassive(key) => match v.passives.iter_mut().find(|(p, _)| p.key == key) {
            Some((_, level)) if *level < data::MAX_PASSIVE_LEVEL => {
                *level += 1;
                true
            }
            _ => false,
        },
        WireChoice::Heal => {
            v.hp = v.stats().max_hp;
            true
        }
    };
    if ok {
        v.pending_levels -= 1;
        v.version += 1;
        v.picks += 1;
    }
    ok
}

/// The run's first frame: the local hero takes up its numbers, and a host seats every guest's
/// hero beside it. A guest's heroes all arrive from the host instead.
#[allow(clippy::too_many_arguments)]
pub(super) fn begin_run(
    mut commands: Commands,
    mut run: ResMut<Run>,
    session: Res<Session>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    store: Res<super::profile::ProfileStore>,
    world: WorldPoint,
    local: Query<Entity, (With<Hero>, With<LocalHero>)>,
) {
    if run.begun || *phase.get() != Phase::Playing {
        return;
    }
    if session.mode == Mode::Client {
        run.begun = true;
        return;
    }
    let Ok(me) = local.single() else {
        return;
    };
    // The local hero brings in what the profile has earned. A guest's hero gets its racials,
    // which its race alone decides; the rest of a guest's profile crossing the wire is its own
    // task.
    let (meta, procs) = store.profile.run_bonuses(run.me.race);
    run.me = std::mem::take(&mut run.me).with_meta(meta, &procs);
    commands.entity(me).insert(HeroState {
        slot: session.slot,
        v: run.me.clone(),
    });
    if session.mode == Mode::Host {
        let center = hero::arena(selection.stage_def());
        let others: Vec<_> = session
            .seats
            .iter()
            .filter(|s| s.slot != session.slot)
            .collect();
        for (i, seat) in others.iter().enumerate() {
            let a = (i as f32 + 1.0) / (others.len() as f32 + 1.0) * std::f32::consts::TAU;
            let mut pos = center + Vec3::new(a.cos(), 0.0, a.sin()) * 3.0;
            if let Some(h) = world.terrain_height_under(pos) {
                pos.y = h;
            }
            let class = &data::CLASSES[seat.class.min(data::CLASSES.len() - 1)];
            let serial = run.guid();
            let e = hero::spawn_hero(
                &mut commands,
                class,
                seat.display,
                seat.race,
                seat.sex,
                serial,
                pos,
            );
            commands.entity(e).insert(HeroState {
                slot: seat.slot,
                v: {
                    let (meta, procs) = data::racial_bonuses(seat.race);
                    Vitals::new(class, seat.race).with_meta(meta, &procs)
                },
            });
        }
    }
    run.begun = true;
}

/// The local player's cards land on the local hero.
pub(super) fn apply_local_picks(
    session: Res<Session>,
    mut picks: ResMut<LocalPicks>,
    mut heroes: Query<&mut HeroState, With<LocalHero>>,
) {
    if !session.simulates() {
        return;
    }
    let Ok(mut me) = heroes.single_mut() else {
        picks.0.clear();
        return;
    };
    for c in picks.0.drain(..) {
        apply_choice(&mut me.v, &c);
    }
}

/// The HUD's view is the local hero, except the mobility state, which the view owns: the local
/// player casts it, and the run reads it from here.
pub(super) fn sync_view(
    session: Res<Session>,
    mut run: ResMut<Run>,
    mut heroes: Query<&mut HeroState, With<LocalHero>>,
) {
    if !session.simulates() || !run.begun {
        return;
    }
    let Ok(mut me) = heroes.single_mut() else {
        return;
    };
    let mobile = run.me.mobile;
    me.v.mobile = mobile;
    run.me = me.v.clone();
    run.me.mobile = mobile;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mage() -> Vitals {
        let class = data::CLASSES
            .iter()
            .find(|c| c.name == "Mage")
            .expect("a mage");
        let mut v = Vitals::new(class, 1);
        v.pending_levels = 10;
        v
    }

    #[test]
    fn a_card_lands_once_and_counts() {
        let mut v = mage();
        let start = v.abilities[0].def.key.to_string();
        assert!(apply_choice(&mut v, &WireChoice::Upgrade(start.clone())));
        assert_eq!(v.abilities[0].level, 2);
        assert_eq!(v.picks, 1);
        assert_eq!(v.pending_levels, 9);
    }

    #[test]
    fn cards_the_rules_would_never_deal_are_refused() {
        let mut v = mage();
        // Another class's ability.
        assert!(!apply_choice(
            &mut v,
            &WireChoice::NewAbility("whirlwind".into())
        ));
        // An ability not owned cannot be upgraded; an unknown passive cannot be taken.
        assert!(!apply_choice(
            &mut v,
            &WireChoice::Upgrade("fireball".into())
        ));
        assert!(!apply_choice(
            &mut v,
            &WireChoice::NewPassive("nope".into())
        ));
        // Already owned.
        let start = v.abilities[0].def.key.to_string();
        assert!(!apply_choice(&mut v, &WireChoice::NewAbility(start)));
        // No banked level, no card.
        v.pending_levels = 0;
        assert!(!apply_choice(&mut v, &WireChoice::Heal));
        assert_eq!(v.picks, 0);
    }

    #[test]
    fn ranks_and_slots_cap() {
        let mut v = mage();
        v.pending_levels = 100;
        let start = v.abilities[0].def.key.to_string();
        while apply_choice(&mut v, &WireChoice::Upgrade(start.clone())) {}
        assert_eq!(v.abilities[0].level, data::MAX_ABILITY_LEVEL);
        for p in data::PASSIVES {
            apply_choice(&mut v, &WireChoice::NewPassive(p.key.into()));
        }
        assert_eq!(v.passives.len(), data::MAX_PASSIVES);
    }

    #[test]
    fn wire_choices_name_what_the_hud_offered() {
        let v = mage();
        let w = to_wire(Choice::Upgrade(0), &v);
        assert_eq!(w, WireChoice::Upgrade(v.abilities[0].def.key.to_string()));
        assert_eq!(to_wire(Choice::Heal, &v), WireChoice::Heal);
    }
}
