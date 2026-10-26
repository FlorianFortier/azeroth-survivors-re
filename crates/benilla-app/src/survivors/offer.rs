//! The level-up offer, Megabonk-style, as the simulation deals it: three cards per banked level,
//! each with a rarity drawn as it is dealt ([`data::CardRarity`]), worth more ranks the rarer it is.
//! Between picks the hero may reroll the cards, banish one for the rest of the run, or skip the
//! level; it starts with a few of each and earns more as it levels ([`earn_tokens`]).
//!
//! Only solo and the host deal (`deal_offers`), every hero's from its own [`Vitals`]; a guest
//! sees its hero's offer on its `Game::Me` and answers with a `Game::Card` move, which
//! [`apply_action`] takes only if it fits the offer the host dealt. The banished keys stay on
//! the host.

use bevy::prelude::*;
use survivors_net::game::{Card as WireCard, CardAction, Choice as WireChoice};

use super::data::{self, CardRarity};
use super::{HeroState, Owned, Rng, Session, Vitals};

/// One dealt card: what it gives, by key, and the rarity it was drawn at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Dealt {
    pub choice: WireChoice,
    pub rarity: CardRarity,
}

impl Dealt {
    pub fn to_wire(&self) -> WireCard {
        WireCard {
            choice: self.choice.clone(),
            rarity: self.rarity.to_wire(),
        }
    }

    pub fn from_wire(c: &WireCard) -> Option<Self> {
        Some(Self {
            choice: c.choice.clone(),
            rarity: CardRarity::from_wire(c.rarity)?,
        })
    }
}

/// What a banish takes out of a hero's deals: an ability (learning it and its ranks) or a
/// passive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ban {
    Ability(&'static str),
    Passive(&'static str),
}

impl Ban {
    /// What banishing this card would take out; a heal is never banished.
    fn of(choice: &WireChoice) -> Option<Ban> {
        match choice {
            WireChoice::NewAbility(k) | WireChoice::Upgrade(k) => data::ABILITIES
                .iter()
                .find(|d| d.key == k)
                .map(|d| Ban::Ability(d.key)),
            WireChoice::NewPassive(k) | WireChoice::UpPassive(k) => data::PASSIVES
                .iter()
                .find(|p| p.key == k)
                .map(|p| Ban::Passive(p.key)),
            WireChoice::Heal => None,
        }
    }
}

/// Every card the hero could be dealt now, and how often: its ranks to come, the class's spells
/// it lacks while a slot is free, its passives' ranks, new passives; never what it banished.
fn pool(v: &Vitals) -> Vec<(WireChoice, f32)> {
    let banned = |b: Ban| v.banned.contains(&b);
    let mut pool = Vec::new();
    for o in &v.abilities {
        if o.level < data::MAX_ABILITY_LEVEL && !banned(Ban::Ability(o.def.key)) {
            pool.push((WireChoice::Upgrade(o.def.key.to_string()), 1.2));
        }
    }
    if v.abilities.len() < data::MAX_ABILITIES {
        for &key in v.class.pool {
            let key = data::ability(key).key;
            if !v.abilities.iter().any(|o| o.def.key == key) && !banned(Ban::Ability(key)) {
                pool.push((WireChoice::NewAbility(key.to_string()), 1.0));
            }
        }
    }
    for (p, level, _) in &v.passives {
        if *level < data::MAX_PASSIVE_LEVEL && !banned(Ban::Passive(p.key)) {
            pool.push((WireChoice::UpPassive(p.key.to_string()), 0.8));
        }
    }
    if v.passives.len() < data::MAX_PASSIVES {
        for p in data::PASSIVES {
            if !v.passives.iter().any(|(q, ..)| q.key == p.key) && !banned(Ban::Passive(p.key)) {
                pool.push((WireChoice::NewPassive(p.key.to_string()), 0.5));
            }
        }
    }
    pool
}

/// `n` cards from the hero's pool, none of them in `besides`, each at a drawn rarity; a heal
/// when nothing is left to deal.
pub(super) fn deal(v: &Vitals, rng: &mut Rng, n: usize, besides: &[Dealt]) -> Vec<Dealt> {
    let mut pool: Vec<_> = pool(v)
        .into_iter()
        .filter(|(c, _)| !besides.iter().any(|d| d.choice == *c))
        .collect();
    let mut out = Vec::with_capacity(n);
    while out.len() < n && !pool.is_empty() {
        let total: f32 = pool.iter().map(|(_, w)| w).sum();
        let mut at = rng.f32() * total;
        let mut idx = pool.len() - 1;
        for (i, (_, w)) in pool.iter().enumerate() {
            if at < *w {
                idx = i;
                break;
            }
            at -= w;
        }
        let choice = pool.swap_remove(idx).0;
        out.push(Dealt {
            choice,
            rarity: CardRarity::draw(rng.f32()),
        });
    }
    if out.is_empty() && !besides.iter().any(|d| d.choice == WireChoice::Heal) {
        out.push(Dealt {
            choice: WireChoice::Heal,
            rarity: CardRarity::Common,
        });
    }
    out
}

/// One more of a reroll, a banish or a skip every [`data::CARD_TOKEN_LEVELS`] levels, in turn;
/// call it as the hero reaches a level.
pub(super) fn earn_tokens(v: &mut Vitals) {
    if v.level == 0 || v.level % data::CARD_TOKEN_LEVELS != 0 {
        return;
    }
    let turn = (v.level / data::CARD_TOKEN_LEVELS - 1) % 3;
    let held = match turn {
        0 => &mut v.banishes,
        1 => &mut v.skips,
        _ => &mut v.rerolls,
    };
    *held = (*held + 1).min(data::MAX_CARD_TOKENS);
}

/// Land a card on a hero at `weight` ranks' worth, if it is one the rules could deal: an ability
/// of its class it lacks with a slot free, a rank below the cap, a passive it may take.
fn land(v: &mut Vitals, choice: &WireChoice, weight: f32) -> bool {
    match choice {
        WireChoice::NewAbility(key) => {
            let fits = v.class.pool.contains(&key.as_str())
                && v.abilities.len() < data::MAX_ABILITIES
                && !v.abilities.iter().any(|o| o.def.key == key);
            if fits {
                v.abilities
                    .push(Owned::learned(data::ability(key), weight, 0.3));
            }
            fits
        }
        WireChoice::Upgrade(key) => match v.abilities.iter_mut().find(|o| o.def.key == key) {
            Some(o) if o.level < data::MAX_ABILITY_LEVEL => {
                o.rank_up(weight);
                true
            }
            _ => false,
        },
        WireChoice::NewPassive(key) => match data::PASSIVES.iter().find(|p| p.key == key) {
            Some(p)
                if v.passives.len() < data::MAX_PASSIVES
                    && !v.passives.iter().any(|(q, ..)| q.key == p.key) =>
            {
                v.passives.push((p, 1, weight));
                true
            }
            _ => false,
        },
        WireChoice::UpPassive(key) => match v.passives.iter_mut().find(|(p, ..)| p.key == key) {
            Some((_, level, power)) if *level < data::MAX_PASSIVE_LEVEL => {
                *level += 1;
                *power += weight;
                true
            }
            _ => false,
        },
        WireChoice::Heal => {
            v.hp = v.stats().max_hp;
            true
        }
    }
}

/// A move at the cards, if the hero may make it: a pick of a card it was dealt, at the rarity it
/// was dealt; a reroll, banish or skip it has in hand. Every move taken counts on
/// [`Vitals::picks`], so a guest knows which of its moves the host's numbers include.
pub(super) fn apply_action(v: &mut Vitals, action: &CardAction, rng: &mut Rng) -> bool {
    if v.pending_levels == 0 {
        return false;
    }
    let dealt = |c: &WireCard| Dealt::from_wire(c).filter(|d| v.offer.contains(d));
    let ok = match action {
        CardAction::Pick(c) => match dealt(c) {
            Some(d) if land(v, &d.choice, d.rarity.weight()) => {
                v.pending_levels -= 1;
                v.offer.clear();
                true
            }
            _ => false,
        },
        CardAction::Reroll => {
            if v.rerolls == 0 || v.offer.is_empty() {
                false
            } else {
                v.rerolls -= 1;
                v.offer = deal(v, rng, data::CARDS_DEALT, &[]);
                true
            }
        }
        CardAction::Banish(c) => match dealt(c) {
            Some(d) if v.banishes > 0 => match Ban::of(&d.choice) {
                Some(ban) => {
                    v.banishes -= 1;
                    v.banned.push(ban);
                    v.offer.retain(|o| *o != d);
                    let more = deal(v, rng, 1, &v.offer.clone());
                    v.offer.extend(more);
                    true
                }
                None => false,
            },
            _ => false,
        },
        CardAction::Skip => {
            if v.skips == 0 {
                false
            } else {
                v.skips -= 1;
                v.pending_levels -= 1;
                v.offer.clear();
                true
            }
        }
    };
    if ok {
        v.version += 1;
        v.picks += 1;
    }
    ok
}

/// Every hero with a level banked and no cards in front of it is dealt its offer.
pub(super) fn deal_offers(
    session: Res<Session>,
    mut rng: ResMut<Rng>,
    mut heroes: Query<&mut HeroState>,
) {
    if !session.simulates() {
        return;
    }
    for mut h in &mut heroes {
        if h.pending_levels > 0 && h.offer.is_empty() && h.alive() {
            // The Opening Hand perk's cards go on the first deal.
            let n = data::CARDS_DEALT + usize::from(h.v.opening_hand);
            h.v.opening_hand = 0;
            let offer = deal(&h.v, &mut rng, n, &[]);
            h.v.offer = offer;
        } else if h.pending_levels == 0 && !h.offer.is_empty() {
            h.v.offer.clear();
        }
    }
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

    fn dealt(v: &mut Vitals, rng: &mut Rng) -> Dealt {
        v.offer = deal(v, rng, data::CARDS_DEALT, &[]);
        v.offer[0].clone()
    }

    #[test]
    fn three_different_cards_are_dealt() {
        let mut rng = Rng::seeded(4);
        let v = mage();
        for _ in 0..50 {
            let offer = deal(&v, &mut rng, data::CARDS_DEALT, &[]);
            assert_eq!(offer.len(), 3);
            for (i, a) in offer.iter().enumerate() {
                assert!(offer[i + 1..].iter().all(|b| b.choice != a.choice));
            }
        }
    }

    #[test]
    fn a_pick_lands_at_its_rarity_and_counts() {
        let mut v = mage();
        let key = v.abilities[0].def.key.to_string();
        let card = Dealt {
            choice: WireChoice::Upgrade(key),
            rarity: CardRarity::Epic,
        };
        v.offer = vec![card.clone()];
        assert!(apply_action(
            &mut v,
            &CardAction::Pick(card.to_wire()),
            &mut Rng::seeded(1)
        ));
        assert_eq!(v.abilities[0].level, 2);
        assert!((v.abilities[0].power - 3.0).abs() < 1e-5, "1 + Epic's 2");
        assert_eq!(v.picks, 1);
        assert_eq!(v.pending_levels, 9);
        assert!(v.offer.is_empty(), "the next level is dealt afresh");
    }

    #[test]
    fn a_rarer_rank_hits_harder_but_ranks_the_same() {
        let mut common = mage();
        let mut legendary = mage();
        common.abilities[0].rank_up(CardRarity::Common.weight());
        legendary.abilities[0].rank_up(CardRarity::Legendary.weight());
        let (c, l) = (&common.abilities[0], &legendary.abilities[0]);
        assert_eq!(c.level, l.level);
        assert_eq!(c.count(), l.count());
        assert!(l.damage_mult() > c.damage_mult());
        // A Common rank is a rank as the mode always counted it.
        assert!((c.damage_mult() - 1.3).abs() < 1e-5);
    }

    #[test]
    fn only_the_dealt_card_at_its_dealt_rarity_is_taken() {
        let mut rng = Rng::seeded(9);
        let mut v = mage();
        let card = dealt(&mut v, &mut rng);
        // The same card claimed rarer than it was dealt.
        let mut forged = card.to_wire();
        forged.rarity = CardRarity::Legendary.to_wire();
        if card.rarity != CardRarity::Legendary {
            assert!(!apply_action(&mut v, &CardAction::Pick(forged), &mut rng));
        }
        // A card never dealt.
        let other = WireCard {
            choice: WireChoice::NewAbility("whirlwind".into()),
            rarity: 0,
        };
        assert!(!apply_action(&mut v, &CardAction::Pick(other), &mut rng));
        assert_eq!(v.picks, 0);
        assert!(apply_action(
            &mut v,
            &CardAction::Pick(card.to_wire()),
            &mut rng
        ));
        // No banked level, no move.
        v.pending_levels = 0;
        v.offer = vec![card.clone()];
        assert!(!apply_action(&mut v, &CardAction::Skip, &mut rng));
    }

    #[test]
    fn rerolls_deal_again_while_any_is_left() {
        let mut rng = Rng::seeded(2);
        let mut v = mage();
        v.rerolls = 1;
        dealt(&mut v, &mut rng);
        assert!(apply_action(&mut v, &CardAction::Reroll, &mut rng));
        assert_eq!(v.offer.len(), 3);
        assert_eq!(v.rerolls, 0);
        assert!(!apply_action(&mut v, &CardAction::Reroll, &mut rng));
        assert_eq!(v.pending_levels, 10, "a reroll spends no level");
    }

    #[test]
    fn a_banished_card_is_never_dealt_again() {
        let mut rng = Rng::seeded(5);
        let mut v = mage();
        v.banishes = 1;
        v.offer = deal(&v, &mut rng, data::CARDS_DEALT, &[]);
        let card = v
            .offer
            .iter()
            .find(|d| d.choice != WireChoice::Heal)
            .cloned()
            .expect("a card to banish");
        assert!(apply_action(
            &mut v,
            &CardAction::Banish(card.to_wire()),
            &mut rng
        ));
        assert_eq!(v.banishes, 0);
        assert_eq!(v.offer.len(), 3, "its place is dealt again");
        assert!(!v.offer.contains(&card));
        let ban = Ban::of(&card.choice).unwrap();
        for _ in 0..200 {
            for d in deal(&v, &mut rng, data::CARDS_DEALT, &[]) {
                assert_ne!(Ban::of(&d.choice).as_ref(), Some(&ban));
            }
        }
        // None left in hand.
        let next = v.offer[0].to_wire();
        assert!(!apply_action(&mut v, &CardAction::Banish(next), &mut rng));
    }

    #[test]
    fn a_skip_lets_a_level_go_by() {
        let mut rng = Rng::seeded(3);
        let mut v = mage();
        v.skips = 1;
        dealt(&mut v, &mut rng);
        let before = (v.abilities.len(), v.passives.len());
        assert!(apply_action(&mut v, &CardAction::Skip, &mut rng));
        assert_eq!(v.pending_levels, 9);
        assert_eq!((v.abilities.len(), v.passives.len()), before);
        assert!(!apply_action(&mut v, &CardAction::Skip, &mut rng));
    }

    #[test]
    fn tokens_come_with_the_levels_in_turn() {
        let mut v = mage();
        let start = (v.rerolls, v.banishes, v.skips);
        for level in 2..=15 {
            v.level = level;
            earn_tokens(&mut v);
        }
        assert_eq!(v.banishes, start.1 + 1);
        assert_eq!(v.skips, start.2 + 1);
        assert_eq!(v.rerolls, start.0 + 1);
        for level in 16..=500 {
            v.level = level;
            earn_tokens(&mut v);
        }
        assert_eq!(v.rerolls, data::MAX_CARD_TOKENS);
    }

    #[test]
    fn with_everything_learned_the_card_is_a_heal() {
        let mut rng = Rng::seeded(8);
        let mut v = mage();
        v.pending_levels = 500;
        for _ in 0..400 {
            let offer = deal(&v, &mut rng, data::CARDS_DEALT, &[]);
            v.offer = offer.clone();
            if offer[0].choice == WireChoice::Heal {
                break;
            }
            apply_action(&mut v, &CardAction::Pick(offer[0].to_wire()), &mut rng);
        }
        let last = deal(&v, &mut rng, data::CARDS_DEALT, &[]);
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].choice, WireChoice::Heal);
    }
}
