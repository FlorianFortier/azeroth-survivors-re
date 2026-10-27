//! The field's small trade, Megabonk-style: a shady merchant standing somewhere on the
//! battleground with three run items for the run's gold (restocked now and then, dearer with each
//! one bought), and jars and crates about the field that break when a hero walks into them, for a
//! few coins or a bite to eat.
//!
//! Only solo and the host tend them ([`tend_market`], in `SimSet`). The merchant is a unit
//! (`FieldMark::Merchant`, `Role::Merchant` on the wire), the jars and crates game objects, and the
//! wares [`Run::shop`] on `Game::Run`, so guests see all of it. A purchase is asked for through
//! [`BuyQueue`]: the local player's own on solo and the host, a guest's from its `Game::Buy`; the
//! host takes it only for a hero standing at the merchant with the gold, for a ware still there.

use benilla_world::world_point::WorldPoint;
use bevy::prelude::*;

use super::data::{self, RunItemDef};
use super::enemies::{self, GemKind};
use super::hero::Hero;
use super::hud::{HudFont, HudRoot, SpellIcon};
use super::lang::{tr, trf};
use super::treasure::{self, FieldMark};
use super::units::{self, flat, UnitSpec};
use super::{HeroState, LocalHero, Phase, Rng, Run, Selection, Session};

/// One of the merchant's wares, as every machine sees it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Ware {
    pub item: &'static RunItemDef,
    pub price: u32,
    pub sold: bool,
}

/// The merchant and the breakables of the run in play, as the host tends them.
#[derive(Resource, Default)]
pub(super) struct Market {
    /// The run and stage it was laid out for.
    run_id: Option<(u32, u8)>,
    pos: Vec3,
    restock: f32,
    /// Wares bought this run, for the markup.
    bought: u32,
    /// Seconds until each broken jar or crate stands again elsewhere.
    returning: Vec<f32>,
}

/// Purchases asked for: `(hero's seat, ware's place, its item's key)`. The simulation takes them
/// on solo and the host; a guest's go to the host.
#[derive(Resource, Default)]
pub(super) struct BuyQueue(pub Vec<(u8, u8, String)>);

/// A jar (gold) or a crate (a heal, or a coin).
#[derive(Component)]
pub(super) struct Breakable {
    crate_: bool,
}

/// The merchant's body: the Rogue trainer's look, a shady sort.
fn merchant_display() -> u32 {
    data::CLASSES
        .iter()
        .find(|c| c.name == "Rogue")
        .and_then(|c| c.looks.first().copied())
        .unwrap_or(data::CLASSES[0].looks[0])
}

/// Three wares of different items, each drawn as a chest draws one, at its rarity's price.
pub(super) fn stock(rng: &mut Rng, bought: u32) -> Vec<Ware> {
    let mut out: Vec<Ware> = Vec::with_capacity(data::SHOP_SLOTS);
    let mut tries = 0;
    while out.len() < data::SHOP_SLOTS && tries < 64 {
        tries += 1;
        let item = data::draw_run_item(rng.f32(), rng.f32());
        if out.iter().any(|w| w.item.key == item.key) {
            continue;
        }
        out.push(Ware {
            item,
            price: data::shop_price(item.rarity, bought),
            sold: false,
        });
    }
    out
}

/// Sell `hero` the ware at `slot` of `shop` if it may buy it: still there, the item named
/// `key`, the gold in hand, room for one more stack (and for its kind, five kinds at most).
/// Returns the item bought.
pub(super) fn buy(
    hero: &mut HeroState,
    shop: &mut [Ware],
    slot: usize,
    key: &str,
) -> Option<&'static RunItemDef> {
    let ware = shop.get_mut(slot)?;
    if ware.sold || ware.item.key != key || hero.gold + 1e-3 < ware.price as f32 {
        return None;
    }
    let item = ware.item;
    let held = hero
        .run_items
        .iter()
        .find(|(i, _)| i.key == item.key)
        .map_or(0, |(_, n)| *n);
    // A full stack, or a sixth kind, is not sold.
    if held >= data::MAX_RUN_ITEM_STACKS
        || (held == 0 && hero.run_items.len() >= data::MAX_RUN_ITEM_KINDS)
    {
        return None;
    }
    hero.gold -= ware.price as f32;
    ware.sold = true;
    match hero.run_items.iter_mut().find(|(i, _)| i.key == item.key) {
        Some((_, n)) => *n += 1,
        None => hero.run_items.push((item, 1)),
    }
    hero.version += 1;
    Some(item)
}

fn spawn_breakable(commands: &mut Commands, run: &mut Run, rng: &mut Rng, pos: Vec3) {
    let crate_ = rng.f32() < 0.4;
    let (display, scale) = if crate_ {
        treasure::crate_look()
    } else {
        treasure::jar_look()
    };
    let e = units::spawn_object(commands, run.guid(), display, scale, pos);
    commands
        .entity(e)
        .insert((Breakable { crate_ }, super::Pickup));
}

/// Lay out the merchant and the breakables at a run's start, restock the wares, break what a
/// hero walks into, and sell what is asked for.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn tend_market(
    mut commands: Commands,
    time: Res<Time>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    mut rng: ResMut<Rng>,
    mut market: ResMut<Market>,
    mut queue: ResMut<BuyQueue>,
    world: WorldPoint,
    breakables: Query<(Entity, &Transform, &Breakable)>,
    mut heroes: Query<(&Transform, &mut HeroState), (With<Hero>, Without<Breakable>)>,
) {
    if !run.begun {
        queue.0.clear();
        return;
    }
    let dt = time.delta_secs();
    let map = selection.stage_def();
    let center = super::hero::arena(map);

    // A new run: the merchant takes its place with its wares, and the jars and crates theirs.
    if market.run_id != Some((run.run_id, run.leg)) {
        *market = Market {
            run_id: Some((run.run_id, run.leg)),
            restock: data::SHOP_RESTOCK_SECS,
            ..Market::default()
        };
        let a = rng.range(0.0, std::f32::consts::TAU);
        let pos = treasure::spot(&world, center, map.leash, a, 0.5);
        market.pos = pos;
        let serial = run.guid();
        let e = units::spawn_unit(
            &mut commands,
            &UnitSpec {
                serial,
                display: merchant_display(),
                scale: 1.0,
                level: 60,
                faction: units::FACTION_FRIENDLY,
                bytes0: 1 << 8,
                weapons: &[],
                sheath: 0,
            },
            pos,
            units::yaw_toward(center - pos),
        );
        commands.entity(e).insert(FieldMark::Merchant);
        run.shop = stock(&mut rng, 0);
        for _ in 0..data::BREAKABLES {
            let a = rng.range(0.0, std::f32::consts::TAU);
            let t = rng.range(0.15, 0.95);
            let pos = treasure::spot(&world, center, map.leash, a, t);
            spawn_breakable(&mut commands, &mut run, &mut rng, pos);
        }
    }

    // The merchant restocks now and then.
    market.restock -= dt;
    if market.restock <= 0.0 {
        market.restock = data::SHOP_RESTOCK_SECS;
        let bought = market.bought;
        run.shop = stock(&mut rng, bought);
        run.banner = Some(("The merchant has new wares.".into(), 3.0));
    }

    // A hero walking into a jar or a crate breaks it.
    for (e, tf, b) in &breakables {
        let at = tf.translation;
        let near = heroes
            .iter()
            .any(|(t, h)| h.alive() && flat(t.translation - at).length() <= data::BREAK_REACH);
        if !near {
            continue;
        }
        commands.entity(e).despawn();
        market.returning.push(data::BREAKABLE_RESPAWN_SECS);
        let kind = if b.crate_ && rng.f32() < data::CRATE_HEAL_ODDS {
            GemKind::Heal(data::CRATE_HEAL)
        } else if b.crate_ {
            GemKind::Gold(1)
        } else {
            let (lo, hi) = data::JAR_GOLD;
            GemKind::Gold(lo + rng.index((hi - lo + 1) as usize) as u32)
        };
        enemies::spawn_gem(&mut commands, run.guid(), at, kind);
    }
    let mut back = 0;
    market.returning.retain_mut(|t| {
        *t -= dt;
        if *t <= 0.0 {
            back += 1;
            false
        } else {
            true
        }
    });
    for _ in 0..back {
        let a = rng.range(0.0, std::f32::consts::TAU);
        let t = rng.range(0.15, 0.95);
        let pos = treasure::spot(&world, center, map.leash, a, t);
        spawn_breakable(&mut commands, &mut run, &mut rng, pos);
    }

    // Purchases, each for a hero at the merchant with the gold.
    let at = market.pos;
    for (seat, slot, key) in std::mem::take(&mut queue.0) {
        let Some((t, mut hero)) = heroes.iter_mut().find(|(_, h)| h.slot == seat) else {
            continue;
        };
        if !hero.alive() || flat(t.translation - at).length() > data::MERCHANT_REACH + 1.0 {
            continue;
        }
        let mut shop = std::mem::take(&mut run.shop);
        if let Some(item) = buy(&mut hero, &mut shop, usize::from(slot), &key) {
            market.bought += 1;
            // What is left costs more.
            for w in shop.iter_mut().filter(|w| !w.sold) {
                w.price = data::shop_price(w.item.rarity, market.bought);
            }
            run.banner = Some((format!("Bought {}", item.name), 3.0));
        }
        run.shop = shop;
    }
}

/// The shop panel: shown while the local hero stands at the merchant.
#[derive(Component)]
pub(super) struct ShopPanel;

#[derive(Component)]
pub(super) struct ShopButton(u8);

/// Whether the local hero stands at the merchant, in play.
fn at_merchant(
    run: &Run,
    phase: &Phase,
    hero: Option<Vec3>,
    merchants: impl Iterator<Item = Vec3>,
) -> bool {
    let Some(hero) = hero else { return false };
    *phase == Phase::Playing
        && run.begun
        && !run.shop.is_empty()
        && merchants
            .into_iter()
            .any(|m| flat(m - hero).length() <= data::MERCHANT_REACH)
}

/// Ask for the ware at `slot`: the run takes it (solo, host), or the host does (guest).
fn ask(queue: &mut BuyQueue, session: &Session, run: &Run, slot: usize) {
    if let Some(w) = run.shop.get(slot).filter(|w| !w.sold) {
        queue
            .0
            .push((session.slot, slot as u8, w.item.key.to_string()));
    }
}

/// The shop panel over the HUD while the local hero stands at the merchant: its three wares, each
/// with its icon, name in its rarity's colour and price; a click or 1, 2, 3 buys.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn shop_panel(
    mut commands: Commands,
    run: Res<Run>,
    phase: Res<State<Phase>>,
    session: Res<Session>,
    keys: Res<ButtonInput<KeyCode>>,
    font: Option<Res<HudFont>>,
    roots: Query<Entity, With<HudRoot>>,
    hero: Query<&Transform, With<LocalHero>>,
    merchants: Query<(&Transform, &FieldMark)>,
    mut panels: Query<&mut Visibility, With<ShopPanel>>,
    clicks: Query<(&ShopButton, &Interaction), Changed<Interaction>>,
    mut queue: ResMut<BuyQueue>,
    mut built: Local<Option<Entity>>,
    mut shown: Local<Option<(Vec<(&'static str, u32, bool)>, u32, u32)>>,
) {
    let near = at_merchant(
        &run,
        phase.get(),
        hero.single().ok().map(|t| t.translation),
        merchants
            .iter()
            .filter(|(_, m)| **m == FieldMark::Merchant)
            .map(|(t, _)| t.translation),
    );
    if let Some(panel) = *built {
        if let Ok(mut vis) = panels.get_mut(panel) {
            let want = if near {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            };
            if *vis != want {
                *vis = want;
            }
        }
    }
    if !near {
        return;
    }
    // A click or a key buys.
    for (b, i) in &clicks {
        if *i == Interaction::Pressed {
            ask(&mut queue, &session, &run, usize::from(b.0));
        }
    }
    for (slot, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
        .into_iter()
        .enumerate()
    {
        if keys.just_pressed(key) {
            ask(&mut queue, &session, &run, slot);
        }
    }

    // Rebuilt when the wares, the gold in hand or the language change.
    let key = (
        run.shop
            .iter()
            .map(|w| (w.item.key, w.price, w.sold))
            .collect::<Vec<_>>(),
        run.gold.floor() as u32,
        super::lang::generation(),
    );
    if shown.as_ref() == Some(&key) && built.is_some() {
        return;
    }
    let (Some(font), Ok(root)) = (font, roots.single()) else {
        return;
    };
    *shown = Some(key);
    if let Some(old) = built.take() {
        commands.entity(old).despawn();
    }
    let f = &font.0;
    let text = |s: String, size: f32, color: Color| {
        (
            Text::new(s),
            TextFont {
                font: f.clone(),
                font_size: size,
                ..default()
            },
            TextColor(color),
            TextShadow {
                offset: Vec2::splat(1.5),
                color: Color::BLACK,
            },
        )
    };
    let panel = commands
        .spawn((
            ShopPanel,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(50.0),
                bottom: Val::Px(150.0),
                margin: UiRect::left(Val::Px(-260.0)),
                width: Val::Px(520.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(8.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.04, 0.03, 0.88)),
            BorderColor::all(super::hud::GOLD_TONE.with_alpha(0.7)),
        ))
        .with_children(|p| {
            p.spawn(text(
                tr("Shady Merchant").to_string(),
                18.0,
                super::hud::GOLD_TONE,
            ));
            p.spawn(Node {
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                for (i, w) in run.shop.iter().enumerate() {
                    let rc = super::cards::rarity_color(w.item.rarity);
                    let afford = run.gold + 1e-3 >= w.price as f32;
                    row.spawn((
                        ShopButton(i as u8),
                        Button,
                        Interaction::default(),
                        super::hud::TipFor::Ware(i),
                        Node {
                            width: Val::Px(160.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            row_gap: Val::Px(3.0),
                            padding: UiRect::all(Val::Px(6.0)),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.1, 0.08, 0.05, 0.9)),
                        BorderColor::all(if w.sold {
                            Color::srgba(1.0, 1.0, 1.0, 0.15)
                        } else {
                            rc
                        }),
                    ))
                    .with_children(|c| {
                        c.spawn((
                            Node {
                                width: Val::Px(34.0),
                                height: Val::Px(34.0),
                                ..default()
                            },
                            ImageNode {
                                color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                                ..default()
                            },
                            SpellIcon(w.item.spell),
                        ));
                        c.spawn(text(format!("{}  {}", i + 1, w.item.name), 13.0, rc));
                        let (line, color) = if w.sold {
                            (tr("Sold").to_string(), super::hud::DIM_GREY)
                        } else if afford {
                            (
                                trf("{} gold", &[&w.price.to_string()]),
                                super::hud::GOLD_TONE,
                            )
                        } else {
                            (
                                trf("{} gold", &[&w.price.to_string()]),
                                Color::srgb(0.85, 0.3, 0.25),
                            )
                        };
                        c.spawn(text(line, 13.0, color));
                    });
                }
            });
            p.spawn(text(
                tr("Click a ware, or press 1, 2 or 3 to buy").to_string(),
                12.0,
                super::hud::DIM_GREY,
            ));
        })
        .id();
    commands.entity(root).add_child(panel);
    *built = Some(panel);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::survivors::Vitals;

    fn hero(gold: f32) -> HeroState {
        let mut v = Vitals::new(&data::CLASSES[0], 1);
        v.gold = gold;
        HeroState { slot: 0, v }
    }

    #[test]
    fn the_merchant_stocks_three_different_wares_at_their_price() {
        let mut rng = Rng::seeded(4);
        for _ in 0..40 {
            let s = stock(&mut rng, 2);
            assert_eq!(s.len(), data::SHOP_SLOTS);
            for (i, a) in s.iter().enumerate() {
                assert!(s[i + 1..].iter().all(|b| b.item.key != a.item.key));
                assert_eq!(a.price, data::shop_price(a.item.rarity, 2));
                assert!(!a.sold);
            }
        }
    }

    #[test]
    fn a_ware_sells_once_for_its_gold_and_only_by_its_name() {
        let mut shop = stock(&mut Rng::seeded(9), 0);
        let price = shop[1].price as f32;
        let key = shop[1].item.key;
        // Too poor.
        let mut poor = hero(price - 1.0);
        assert!(buy(&mut poor, &mut shop, 1, key).is_none());
        // Another item's name at that place.
        let mut rich = hero(price + 10.0);
        let other = shop[0].item.key;
        assert!(buy(&mut rich, &mut shop, 1, other).is_none());
        assert_eq!(buy(&mut rich, &mut shop, 1, key).map(|i| i.key), Some(key));
        assert!((rich.gold - 10.0).abs() < 1e-3);
        assert_eq!(rich.run_items.len(), 1);
        assert!(shop[1].sold);
        // Gone.
        rich.gold = 1000.0;
        assert!(buy(&mut rich, &mut shop, 1, key).is_none());
        // No such place.
        assert!(buy(&mut rich, &mut shop, 7, key).is_none());
    }

    #[test]
    fn a_full_stack_is_not_sold_more() {
        let mut shop = stock(&mut Rng::seeded(3), 0);
        let item = shop[0].item;
        let mut h = hero(10_000.0);
        h.run_items.push((item, data::MAX_RUN_ITEM_STACKS));
        assert!(buy(&mut h, &mut shop, 0, item.key).is_none());
        assert!(!shop[0].sold);
    }

    #[test]
    fn a_sixth_kind_is_not_sold() {
        let mut shop = stock(&mut Rng::seeded(5), 0);
        let item = shop[0].item;
        let mut h = hero(10_000.0);
        for other in data::RUN_ITEMS
            .iter()
            .filter(|i| i.key != item.key)
            .take(data::MAX_RUN_ITEM_KINDS)
        {
            h.run_items.push((other, 1));
        }
        assert!(buy(&mut h, &mut shop, 0, item.key).is_none());
    }

    #[test]
    fn the_panel_shows_only_at_the_merchant_in_play() {
        let mut run = Run {
            begun: true,
            ..Run::default()
        };
        run.shop = stock(&mut Rng::seeded(1), 0);
        let m = [Vec3::new(10.0, 0.0, 0.0)];
        let near = Some(Vec3::new(10.0 + data::MERCHANT_REACH - 0.5, 0.0, 0.0));
        let far = Some(Vec3::new(10.0 + data::MERCHANT_REACH + 1.0, 0.0, 0.0));
        assert!(at_merchant(&run, &Phase::Playing, near, m.into_iter()));
        assert!(!at_merchant(&run, &Phase::Playing, far, m.into_iter()));
        assert!(!at_merchant(&run, &Phase::LevelUp, near, m.into_iter()));
        assert!(!at_merchant(&run, &Phase::Playing, None, m.into_iter()));
    }
}
