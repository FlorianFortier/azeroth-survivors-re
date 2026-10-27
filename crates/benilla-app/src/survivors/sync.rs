//! Between the run's heroes and the local player's view of them: seating the heroes when a run
//! begins, landing the local player's moves at the level-up cards on its hero (`offer.rs` deals
//! and checks them), and mirroring the local hero into [`Run`] for the HUD.
//!
//! The view is optimistic: a move made here shows at once, and the authoritative hero catches
//! up the same frame (solo, host) or a round trip later (guest, see `net::client`).

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;
use survivors_net::game::CardAction;

use super::data;
use super::hero::{self, Hero};
use super::{HeroState, LocalHero, Mode, Phase, Rng, Run, Selection, Session, Vitals};

/// The local player's moves at the cards, and the blessings it took from a Shrine of Power (by
/// their bonus, as the wire writes it), for the run to apply (solo, host) or to send (guest).
#[derive(Resource, Default)]
pub(super) struct LocalPicks(pub Vec<CardAction>, pub Vec<survivors_net::game::WireBonus>);

/// The run's first frame: the local hero takes up its numbers, and a host seats every guest's
/// hero beside it. A guest's heroes all arrive from the host instead.
#[allow(clippy::too_many_arguments)]
pub(super) fn begin_run(
    mut commands: Commands,
    mut run: ResMut<Run>,
    mut session: ResMut<Session>,
    phase: Res<State<Phase>>,
    selection: Res<Selection>,
    store: Res<super::profile::ProfileStore>,
    catalog: Option<Res<crate::entities::CharCreate>>,
    talents: Option<Res<crate::ui_talent::Talents>>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    world: WorldPoint,
    local: Query<Entity, (With<Hero>, With<LocalHero>)>,
) {
    if run.begun || *phase.get() != Phase::Playing {
        return;
    }
    if session.mode == Mode::Client {
        // The host plays this hero with what the loadout carried; the view here takes the same
        // numbers, so its bars and tooltips read as the host's.
        let (meta, procs) = store.profile.run_bonuses(
            run.me.race,
            run.me.class,
            talents.as_deref().map(|t| &t.catalog),
        );
        let hp = run.me.hp;
        run.me = std::mem::take(&mut run.me).with_meta(meta, &procs);
        run.me.hp = hp;
        run.begun = true;
        return;
    }
    let Ok(me) = local.single() else {
        return;
    };
    // Alone, the run is played at the difficulty picked, if the zone has opened it.
    if session.mode == Mode::Solo {
        let zone = selection.stage_def().name;
        let d = selection.difficulty.min(store.profile.hardest_open(zone));
        session.rules = data::Rules::solo_at(d);
        run.difficulty = d;
    }
    // The local hero brings in what the profile has earned; a guest's, its racials and what its
    // loadout carried (gear, collection, talents).
    let (meta, procs) = store.profile.run_bonuses(
        run.me.race,
        run.me.class,
        talents.as_deref().map(|t| &t.catalog),
    );
    run.me = std::mem::take(&mut run.me).with_meta(meta, &procs);
    super::rewards::apply_perks(&mut run.me, store.profile.perk_ranks());
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
            let body = super::outfit::body(
                class,
                seat.race,
                seat.sex,
                seat.appearance,
                seat.display,
                &seat_gear(seat),
                &super::profile::cosmetics(
                    seat.brings.looks.iter().map(String::as_str),
                    Some(&seat.brings.tabard),
                ),
                catalog.as_deref().map(|c| &c.0),
                displays.as_deref().map(|d| &d.catalog),
            );
            let e = hero::spawn_hero(
                &mut commands,
                class,
                &body,
                seat.race,
                seat.sex,
                serial,
                pos,
            );
            commands.entity(e).insert(HeroState {
                slot: seat.slot,
                v: {
                    let (meta, procs) = seat_bonuses(seat);
                    let mut v = Vitals::new(class, seat.race).with_meta(meta, &procs);
                    super::rewards::apply_perks(&mut v, super::rewards::seat_perks(seat));
                    v
                },
            });
        }
    }
    run.begun = true;
}

/// What a guest's hero brings into the host's run: its race's racials and the bonuses its
/// loadout carried.
pub(super) fn seat_bonuses(seat: &super::Seat) -> (Vec<data::Bonus>, Vec<&'static data::ProcDef>) {
    let (mut meta, procs) = data::racial_bonuses(seat.race);
    meta.extend(
        seat.brings
            .bonuses
            .iter()
            .filter_map(|&b| data::bonus_from_wire(b)),
    );
    (meta, procs)
}

/// The gear a guest's loadout names, one piece a slot, names the tables lack left off.
pub(super) fn seat_gear(seat: &super::Seat) -> Vec<&'static data::ItemDef> {
    let mut out: Vec<&'static data::ItemDef> = Vec::new();
    for item in seat.brings.gear.iter().filter_map(|n| data::item(n)) {
        if !out.iter().any(|i| i.slot == item.slot) {
            out.push(item);
        }
    }
    out
}

/// The local player's moves at the cards land on the local hero.
pub(super) fn apply_local_picks(
    session: Res<Session>,
    mut rng: ResMut<Rng>,
    mut picks: ResMut<LocalPicks>,
    mut heroes: Query<&mut HeroState, With<LocalHero>>,
) {
    if !session.simulates() {
        return;
    }
    let Ok(mut me) = heroes.single_mut() else {
        picks.0.clear();
        picks.1.clear();
        return;
    };
    for a in picks.0.drain(..) {
        super::offer::apply_action(&mut me.v, &a, &mut rng);
    }
    for b in picks.1.drain(..) {
        super::treasure::take_boon(&mut me.v, b);
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

    #[test]
    fn a_guest_brings_its_loadout_into_the_hosts_run() {
        let helm = data::ITEMS
            .iter()
            .find(|i| i.slot == data::GearSlot::Head)
            .unwrap();
        let other_helm = data::ITEMS
            .iter()
            .filter(|i| i.slot == data::GearSlot::Head)
            .nth(1)
            .unwrap();
        let seat = super::super::Seat {
            slot: 1,
            name: "Guest".into(),
            class: 0,
            display: 1,
            race: 8,
            sex: 0,
            appearance: [0; 5],
            brings: super::super::Brings {
                bonuses: vec![
                    data::bonus_to_wire(&data::Bonus::flat(data::Stat::MaxHp, 25.0)),
                    (200, 0, 5),
                ],
                gear: vec![
                    helm.name.clone(),
                    other_helm.name.clone(),
                    "Thunderfury".into(),
                ],
                perks: vec![("second_thoughts".into(), 9), ("no_such_perk".into(), 1)],
                tabard: "flame".into(),
                looks: vec!["helm_of_wrath".into()],
            },
        };
        let (meta, procs) = seat_bonuses(&seat);
        // The Troll's three racials, then the one bonus the tables know.
        assert_eq!(meta.len(), data::racial_bonuses(8).0.len() + 1);
        assert_eq!(procs.len(), 1);
        let v = Vitals::new(&data::CLASSES[0], 8).with_meta(meta, &procs);
        assert!(v.stats().max_hp > Vitals::new(&data::CLASSES[0], 8).stats().max_hp);
        assert_eq!(seat_gear(&seat), [helm]);
        // Its perks: a rank past the most is held to it, an unknown perk is dropped.
        let mut v = v;
        let rerolls = v.rerolls;
        crate::survivors::rewards::apply_perks(
            &mut v,
            crate::survivors::rewards::seat_perks(&seat),
        );
        let most = data::perk("second_thoughts").unwrap().max_rank;
        assert_eq!(v.rerolls, rerolls + most);
    }
}
