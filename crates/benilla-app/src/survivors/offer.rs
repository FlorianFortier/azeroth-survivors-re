//! The level-up offer, Megabonk-style, as the simulation deals it: [`data::CARDS_DEALT`] spell
//! cards per banked level, each at a rarity drawn as it is dealt ([`data::CardRarity`]). A card
//! grows one stat of its spell ([`data::Perk`]), the rarer the more, an Epic two and a Legendary
//! three; a new spell's card at Uncommon or better comes with one. Ranks have no cap, so a build
//! goes where its draws take it. Between picks the hero may reroll the cards or banish one for the
//! rest of the run, both earned as it levels ([`earn_tokens`]), or skip the level for a little
//! gold.
//!
//! Only solo and the host deal (`deal_offers`), every hero's from its own [`Vitals`]; a guest
//! sees its hero's offer on its `Game::Me` and answers with a `Game::Card` move, which
//! [`apply_action`] takes only if it fits the offer the host dealt, perks and all. The banished
//! keys stay on the host.

use bevy::prelude::*;
use survivors_net::game::{Card as WireCard, CardAction, Choice as WireChoice};

use super::data::{self, CardRarity, Perk, PerkKind};
use super::{HeroState, Owned, Rng, Session, Vitals};

/// One dealt card: what it gives, by key, the rarity it was drawn at, and the perks that rarity
/// drew for its spell.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Dealt {
    pub choice: WireChoice,
    pub rarity: CardRarity,
    pub perks: Vec<Perk>,
}

impl Dealt {
    pub fn to_wire(&self) -> WireCard {
        WireCard {
            choice: self.choice.clone(),
            rarity: self.rarity.to_wire(),
            perks: self.perks.iter().map(|p| p.to_wire()).collect(),
        }
    }

    pub fn from_wire(c: &WireCard) -> Option<Self> {
        Some(Self {
            choice: c.choice.clone(),
            rarity: CardRarity::from_wire(c.rarity)?,
            perks: c
                .perks
                .iter()
                .map(|&p| Perk::from_wire(p))
                .collect::<Option<Vec<_>>>()?,
        })
    }
}

/// What a banish takes out of a hero's deals: a spell, learning it and its ranks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Ban {
    Ability(&'static str),
}

impl Ban {
    /// What banishing this card would take out; a heal is never banished.
    fn of(choice: &WireChoice) -> Option<Ban> {
        match choice {
            WireChoice::NewAbility(k) | WireChoice::Upgrade(k) => data::ABILITIES
                .iter()
                .find(|d| d.key == k)
                .map(|d| Ban::Ability(d.key)),
            _ => None,
        }
    }
}

/// Every card the hero could be dealt now, and how often: a rank of each spell it has, and the
/// class's spells it lacks while a slot is free; never what it banished.
fn pool(v: &Vitals) -> Vec<(WireChoice, f32)> {
    let banned = |b: Ban| v.banned.contains(&b);
    let mut pool = Vec::new();
    for o in &v.abilities {
        if !banned(Ban::Ability(o.def.key)) {
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
    pool
}

/// The perks a card for `def` at `rarity` brings: a new spell's one (its damage, else its
/// cooldown) from Uncommon up; a rank's [`data::perks_for`] different stats of those the spell has.
pub(super) fn draw_perks(
    def: &data::AbilityDef,
    new: bool,
    rarity: CardRarity,
    rng: &mut Rng,
) -> Vec<Perk> {
    if new {
        if rarity == CardRarity::Common {
            return Vec::new();
        }
        let kind = if PerkKind::Damage.fits(def) {
            PerkKind::Damage
        } else {
            PerkKind::Cooldown
        };
        return vec![kind.at(rarity)];
    }
    let mut kinds: Vec<PerkKind> = PerkKind::ALL.into_iter().filter(|k| k.fits(def)).collect();
    let mut out = Vec::new();
    while out.len() < data::perks_for(rarity) && !kinds.is_empty() {
        let k = kinds.swap_remove(rng.index(kinds.len()));
        out.push(k.at(rarity));
    }
    out
}

/// `n` cards from the hero's pool, none of them in `besides`, each at a drawn rarity with its
/// perks; a heal when nothing is left to deal.
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
        let rarity = CardRarity::draw(rng.f32());
        let perks = match &choice {
            WireChoice::NewAbility(k) => draw_perks(data::ability(k), true, rarity, rng),
            WireChoice::Upgrade(k) => draw_perks(data::ability(k), false, rarity, rng),
            _ => Vec::new(),
        };
        out.push(Dealt {
            choice,
            rarity,
            perks,
        });
    }
    if out.is_empty() && !besides.iter().any(|d| d.choice == WireChoice::Heal) {
        out.push(Dealt {
            choice: WireChoice::Heal,
            rarity: CardRarity::Common,
            perks: Vec::new(),
        });
    }
    out
}

/// One more of a banish or a reroll every [`data::CARD_TOKEN_LEVELS`] levels, in turn; call it as
/// the hero reaches a level.
pub(super) fn earn_tokens(v: &mut Vitals) {
    if v.level == 0 || v.level % data::CARD_TOKEN_LEVELS != 0 {
        return;
    }
    let held = if (v.level / data::CARD_TOKEN_LEVELS - 1) % 2 == 0 {
        &mut v.banishes
    } else {
        &mut v.rerolls
    };
    *held = (*held + 1).min(data::MAX_CARD_TOKENS);
}

/// Land a card on a hero, if it is one the rules could deal: a spell of its class it lacks with
/// a slot free, or a rank of one it has, each with the card's perks.
fn land(v: &mut Vitals, d: &Dealt) -> bool {
    match &d.choice {
        WireChoice::NewAbility(key) => {
            let fits = v.class.pool.contains(&key.as_str())
                && v.abilities.len() < data::MAX_ABILITIES
                && !v.abilities.iter().any(|o| o.def.key == key);
            if fits {
                v.abilities
                    .push(Owned::learned(data::ability(key), &d.perks, 0.3));
            }
            fits
        }
        WireChoice::Upgrade(key) => match v.abilities.iter_mut().find(|o| o.def.key == key) {
            Some(o) => {
                o.rank_up(&d.perks);
                true
            }
            None => false,
        },
        WireChoice::Heal => {
            v.hp = v.stats().max_hp;
            true
        }
        WireChoice::NewPassive(_) | WireChoice::UpPassive(_) => false,
    }
}

/// A move at the cards, if the hero may make it: a pick of a card it was dealt, as it was dealt;
/// a reroll or banish it has in hand; a skip, always, for [`data::SKIP_GOLD`]. Every move taken
/// counts on [`Vitals::picks`], so a guest knows which of its moves the host's numbers include.
pub(super) fn apply_action(v: &mut Vitals, action: &CardAction, rng: &mut Rng) -> bool {
    if v.pending_levels == 0 {
        return false;
    }
    let dealt = |c: &WireCard| Dealt::from_wire(c).filter(|d| v.offer.contains(d));
    let ok = match action {
        CardAction::Pick(c) => match dealt(c) {
            Some(d) if land(v, &d) => {
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
            let mult = v.stats().gold;
            v.gold += data::SKIP_GOLD as f32 * mult;
            v.pending_levels -= 1;
            v.offer.clear();
            true
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
            let offer = deal(&h.v, &mut rng, data::CARDS_DEALT, &[]);
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
    fn four_different_spell_cards_are_dealt() {
        let mut rng = Rng::seeded(4);
        let v = mage();
        for _ in 0..50 {
            let offer = deal(&v, &mut rng, data::CARDS_DEALT, &[]);
            assert_eq!(offer.len(), 4);
            for (i, a) in offer.iter().enumerate() {
                assert!(offer[i + 1..].iter().all(|b| b.choice != a.choice));
                assert!(matches!(
                    a.choice,
                    WireChoice::NewAbility(_) | WireChoice::Upgrade(_)
                ));
            }
        }
    }

    #[test]
    fn a_rank_grows_as_many_stats_as_its_rarity_draws_and_only_its_own() {
        let mut rng = Rng::seeded(6);
        for def in data::ABILITIES {
            let fit = PerkKind::ALL.iter().filter(|k| k.fits(def)).count();
            for r in CardRarity::ALL {
                let perks = draw_perks(def, false, r, &mut rng);
                assert_eq!(
                    perks.len(),
                    data::perks_for(r).min(fit),
                    "{} {r:?}",
                    def.key
                );
                for (i, p) in perks.iter().enumerate() {
                    assert!(p.kind().fits(def), "{} {p:?}", def.key);
                    assert!(perks[i + 1..].iter().all(|q| q.kind() != p.kind()));
                }
            }
            // A new spell: nothing at Common, one stat above.
            assert!(draw_perks(def, true, CardRarity::Common, &mut rng).is_empty());
            assert_eq!(draw_perks(def, true, CardRarity::Rare, &mut rng).len(), 1);
        }
    }

    #[test]
    fn ranks_have_no_cap_and_each_adds_its_perks() {
        let mut v = mage();
        v.pending_levels = 1000;
        let key = v.abilities[0].def.key.to_string();
        for _ in 0..40 {
            let card = Dealt {
                choice: WireChoice::Upgrade(key.clone()),
                rarity: CardRarity::Rare,
                perks: vec![Perk::Damage(0.25)],
            };
            v.offer = vec![card.clone()];
            assert!(apply_action(
                &mut v,
                &CardAction::Pick(card.to_wire()),
                &mut Rng::seeded(1)
            ));
        }
        assert_eq!(v.abilities[0].level, 41);
        assert!((v.abilities[0].damage_mult() - 11.0).abs() < 1e-3);
    }

    #[test]
    fn a_cooldown_is_cut_by_shares_down_to_a_floor() {
        let mut p = data::Perks::default();
        for _ in 0..100 {
            p = p.with(Perk::Cooldown(0.17));
        }
        assert!((p.cooldown - data::MIN_COOLDOWN_SHARE).abs() < 1e-6);
        let p = data::Perks::default().with(Perk::Cooldown(0.1));
        assert!((p.cooldown - 0.9).abs() < 1e-6);
    }

    #[test]
    fn only_the_dealt_card_with_its_perks_is_taken() {
        let mut rng = Rng::seeded(9);
        let mut v = mage();
        let card = dealt(&mut v, &mut rng);
        // The same card claimed rarer, or with a fatter perk.
        let mut forged = card.to_wire();
        forged.rarity = (forged.rarity + 1) % 5;
        assert!(!apply_action(&mut v, &CardAction::Pick(forged), &mut rng));
        let mut fat = card.to_wire();
        fat.perks = vec![(0, 9.0)];
        if card.perks != vec![Perk::Damage(9.0)] {
            assert!(!apply_action(&mut v, &CardAction::Pick(fat), &mut rng));
        }
        // A card never dealt.
        let other = WireCard {
            choice: WireChoice::NewAbility("whirlwind".into()),
            rarity: 0,
            perks: Vec::new(),
        };
        assert!(!apply_action(&mut v, &CardAction::Pick(other), &mut rng));
        assert_eq!(v.picks, 0);
        assert!(apply_action(
            &mut v,
            &CardAction::Pick(card.to_wire()),
            &mut rng
        ));
    }

    #[test]
    fn rerolls_deal_again_while_any_is_left() {
        let mut rng = Rng::seeded(2);
        let mut v = mage();
        v.rerolls = 1;
        dealt(&mut v, &mut rng);
        assert!(apply_action(&mut v, &CardAction::Reroll, &mut rng));
        assert_eq!(v.offer.len(), data::CARDS_DEALT);
        assert_eq!(v.rerolls, 0);
        assert!(!apply_action(&mut v, &CardAction::Reroll, &mut rng));
        assert_eq!(v.pending_levels, 10, "a reroll spends no level");
    }

    #[test]
    fn a_banished_spell_is_never_dealt_again() {
        let mut rng = Rng::seeded(5);
        let mut v = mage();
        v.banishes = 1;
        v.offer = deal(&v, &mut rng, data::CARDS_DEALT, &[]);
        let card = v.offer[0].clone();
        assert!(apply_action(
            &mut v,
            &CardAction::Banish(card.to_wire()),
            &mut rng
        ));
        assert_eq!(v.banishes, 0);
        assert_eq!(v.offer.len(), data::CARDS_DEALT, "its place is dealt again");
        let ban = Ban::of(&card.choice).unwrap();
        for _ in 0..200 {
            for d in deal(&v, &mut rng, data::CARDS_DEALT, &[]) {
                assert_ne!(Ban::of(&d.choice).as_ref(), Some(&ban));
            }
        }
        let next = v.offer[0].to_wire();
        assert!(!apply_action(&mut v, &CardAction::Banish(next), &mut rng));
    }

    #[test]
    fn a_skip_is_always_there_and_pays_a_little_gold() {
        let mut rng = Rng::seeded(3);
        let mut v = mage();
        dealt(&mut v, &mut rng);
        let before = (v.abilities.len(), v.gold);
        for _ in 0..3 {
            assert!(apply_action(&mut v, &CardAction::Skip, &mut rng));
        }
        assert_eq!(v.pending_levels, 7);
        assert_eq!(v.abilities.len(), before.0);
        assert!((v.gold - before.1 - 3.0 * data::SKIP_GOLD as f32).abs() < 1e-3);
    }

    #[test]
    fn tokens_come_with_the_levels_in_turn() {
        let mut v = mage();
        let start = (v.rerolls, v.banishes);
        for level in 2..=10 {
            v.level = level;
            earn_tokens(&mut v);
        }
        assert_eq!(v.banishes, start.1 + 1);
        assert_eq!(v.rerolls, start.0 + 1);
        for level in 11..=500 {
            v.level = level;
            earn_tokens(&mut v);
        }
        assert_eq!(v.rerolls, data::MAX_CARD_TOKENS);
    }

    #[test]
    fn every_spell_banished_leaves_a_heal() {
        let mut rng = Rng::seeded(8);
        let mut v = mage();
        for d in data::ABILITIES {
            v.banned.push(Ban::Ability(d.key));
        }
        let last = deal(&v, &mut rng, data::CARDS_DEALT, &[]);
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].choice, WireChoice::Heal);
    }
}
