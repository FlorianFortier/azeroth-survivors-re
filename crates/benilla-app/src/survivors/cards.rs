//! The level-up picker, Megabonk-style: four tall cards that flip up out of the deck one after
//! another, tilt toward the cursor, lift under it and burst on the pick. Each card wears the rarity
//! it was drawn at (`offer.rs` deals it) in WoW's item-quality colours: its key and a NEW or RANK
//! badge on top, the spell's icon, its rarity, name and rank, what a new spell does, and in green
//! what the card adds (`+25% damage`, `-10% cooldown`, `+1 targets`); its foot says whether it
//! takes a spell slot. Under each card its Banish button; under the row the hero's spell slots,
//! Skip (for a little gold) and Reroll with what is left of it (R).

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use super::data::{self, CardRarity};
use super::hud::{self, HudFont, Move, Offer, SpellIcon};
use super::lang::{tr, trf};
use super::{Phase, Run};

/// WoW's item-quality colours (`ITEM_QUALITY_COLORS`).
pub(super) const COMMON: Color = Color::srgb(1.0, 1.0, 1.0);
pub(super) const UNCOMMON: Color = Color::srgb(0.118, 1.0, 0.0);
pub(super) const RARE: Color = Color::srgb(0.0, 0.439, 0.867);
pub(super) const EPIC: Color = Color::srgb(0.639, 0.208, 0.933);
pub(super) const LEGENDARY: Color = Color::srgb(1.0, 0.502, 0.0);

const CARD_W: f32 = 228.0;
const CARD_H: f32 = 336.0;
/// Seconds between one card landing and the next.
const DEAL_STAGGER: f32 = 0.09;
const DEAL_SECS: f32 = 0.38;

/// A card's colour, WoW's item-quality colour for its drawn rarity.
pub(super) fn rarity_color(r: CardRarity) -> Color {
    match r {
        CardRarity::Common => COMMON,
        CardRarity::Uncommon => UNCOMMON,
        CardRarity::Rare => RARE,
        CardRarity::Epic => EPIC,
        CardRarity::Legendary => LEGENDARY,
    }
}

pub(super) fn rarity_label(r: CardRarity) -> &'static str {
    tr(match r {
        CardRarity::Common => "Common",
        CardRarity::Uncommon => "Uncommon",
        CardRarity::Rare => "Rare",
        CardRarity::Epic => "Epic",
        CardRarity::Legendary => "Legendary",
    })
}

/// The banish colour: a card about to be banished wears it.
const BANISH: Color = Color::srgb(0.9, 0.18, 0.12);

/// One card's motion: dealt in, tilted to the cursor, lifted under it, burst on the pick.
#[derive(Component)]
pub(super) struct Card {
    index: usize,
    /// Seconds since this card's deal began; negative while it waits its turn.
    t: f32,
    hover: f32,
    tilt: Vec2,
    picked: Option<f32>,
    rarity: CardRarity,
}

/// The rarity glow behind the card, which swells under the cursor.
#[derive(Component)]
pub(super) struct CardGlow;

/// The shine that sweeps across a card as it lands.
#[derive(Component)]
pub(super) struct CardShine(usize);

/// The radiant burst behind the row, turning slowly.
#[derive(Component)]
pub(super) struct Rays;

/// The pending pick: the card bursts, then the choice applies.
#[derive(Resource, Default)]
pub(super) struct PendingPick(Option<(Move, f32)>);

const PICK_SECS: f32 = 0.28;

/// A word in spaced capitals, as the cards' rarity line reads: `R A R E`.
fn spaced(s: &str) -> String {
    s.to_uppercase()
        .chars()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The green of what a card adds.
const GAIN: Color = Color::srgb(0.35, 1.0, 0.35);
const CARD_TEXT: Color = Color::srgb(0.93, 0.91, 0.86);
const CARD_DIM: Color = Color::srgb(0.68, 0.65, 0.6);

/// The Banish button under a card, by its place in the offer.
#[derive(Component)]
pub(super) struct BanishButton(usize);

/// The strip of spell slots in the row under the cards: the hero's spells, empty slots after.
#[derive(Component)]
pub(super) struct SpellStrip;

/// The cards, rebuilt for each offer: each in its column with its Banish button under it, and the
/// strip of the hero's spells under them all.
#[allow(clippy::too_many_arguments)]
pub(super) fn deal_cards(
    mut commands: Commands,
    offer: Res<Offer>,
    run: Res<Run>,
    spells: Option<Res<crate::ui_action::Spells>>,
    font: Res<HudFont>,
    rows: Query<Entity, With<hud::OfferRow>>,
    strips: Query<Entity, With<SpellStrip>>,
) {
    if !offer.is_changed() {
        return;
    }
    let f = &font.0;
    // The hero's spells, a slot each, and the slots still free.
    if let Ok(strip) = strips.single() {
        commands.entity(strip).despawn_related::<Children>();
        commands.entity(strip).with_children(|s| {
            for k in 0..data::MAX_ABILITIES {
                s.spawn((
                    Node {
                        width: Val::Px(34.0),
                        height: Val::Px(34.0),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
                    BorderColor::all(Color::srgba(0.75, 0.6, 0.3, 0.45)),
                ))
                .with_children(|slot| {
                    if let Some(o) = run.abilities.get(k) {
                        slot.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            ImageNode {
                                color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                                ..default()
                            },
                            SpellIcon(o.def.spell),
                        ));
                    }
                });
            }
        });
    }
    let Ok(row) = rows.single() else { return };
    commands.entity(row).despawn_related::<Children>();
    commands.entity(row).with_children(|row| {
        for (i, (dealt, choice)) in offer.cards().iter().enumerate() {
            let rarity = dealt.rarity;
            let card = hud::describe_card(*choice, &dealt.perks, &run, spells.as_deref());
            let rc = rarity_color(rarity);
            let common = rarity == CardRarity::Common;
            let edge = if common {
                Color::srgba(1.0, 1.0, 1.0, 0.35)
            } else {
                rc
            };
            let name_color = if common { Color::WHITE } else { rc };
            let banishable =
                !offer.blessing && matches!(card.badge, hud::Badge::New | hud::Badge::Rank(_));
            row.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    Card {
                        index: i,
                        t: -(i as f32) * DEAL_STAGGER,
                        hover: 0.0,
                        tilt: Vec2::ZERO,
                        picked: None,
                        rarity,
                    },
                    Button,
                    Interaction::default(),
                    RelativeCursorPosition::default(),
                    Node {
                        width: Val::Px(CARD_W),
                        height: Val::Px(CARD_H),
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        padding: UiRect::new(
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(12.0),
                            Val::Px(0.0),
                        ),
                        row_gap: Val::Px(4.0),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(12.0)),
                        ..default()
                    },
                    // A dark card, washed from the top in its rarity's colour.
                    BackgroundGradient::from(LinearGradient::to_bottom(vec![
                        ColorStop::percent(
                            if common {
                                Color::srgba(0.11, 0.1, 0.09, 0.98)
                            } else {
                                rc.mix(&Color::BLACK, 0.78).with_alpha(0.98)
                            },
                            0.0,
                        ),
                        ColorStop::percent(Color::srgba(0.05, 0.045, 0.04, 0.98), 55.0),
                        ColorStop::percent(Color::srgba(0.03, 0.025, 0.02, 0.98), 100.0),
                    ])),
                    BorderColor::all(edge),
                    BoxShadow::new(
                        rc.with_alpha(0.0),
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(26.0),
                    ),
                    UiTransform {
                        translation: Val2::px(0.0, 260.0),
                        scale: Vec2::splat(0.6),
                        rotation: Rot2::degrees(-14.0),
                    },
                    Visibility::Hidden,
                ))
                .with_children(|c| {
                    c.spawn((
                        CardGlow,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            right: Val::Px(0.0),
                            top: Val::Px(0.0),
                            height: Val::Px(150.0),
                            ..default()
                        },
                        BackgroundGradient::from(RadialGradient::new(
                            UiPosition::new(Vec2::new(0.0, -0.1), Val::Auto, Val::Auto),
                            RadialGradientShape::FarthestSide,
                            vec![
                                ColorStop::percent(
                                    rc.with_alpha(if common { 0.12 } else { 0.35 }),
                                    0.0,
                                ),
                                ColorStop::percent(rc.with_alpha(0.0), 100.0),
                            ],
                        )),
                        bevy::ui::FocusPolicy::Pass,
                    ));
                    // The key in its round seat, and the badge: NEW filled, a rank outlined.
                    c.spawn(Node {
                        width: Val::Percent(100.0),
                        justify_content: JustifyContent::SpaceBetween,
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|top| {
                        top.spawn((
                            Node {
                                width: Val::Px(26.0),
                                height: Val::Px(26.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.25)),
                        ))
                        .with_children(|k| {
                            k.spawn(hud::text_bundle(f, format!("{}", i + 1), 14.0, CARD_TEXT));
                        });
                        let (label, filled) = match &card.badge {
                            hud::Badge::New => (tr("NEW").to_string(), true),
                            hud::Badge::Rank(n) => (trf("RANK {}", &[&n.to_string()]), false),
                            hud::Badge::Blessing => (tr("BLESSING").to_string(), true),
                            hud::Badge::Heal => (tr("RESTORE").to_string(), false),
                        };
                        let fill = if common {
                            Color::srgb(0.92, 0.92, 0.92)
                        } else {
                            rc
                        };
                        top.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BackgroundColor(if filled { fill } else { Color::NONE }),
                            BorderColor::all(fill),
                        ))
                        .with_children(|b| {
                            b.spawn(hud::text_bundle(
                                f,
                                label,
                                12.0,
                                if filled {
                                    Color::srgb(0.05, 0.05, 0.05)
                                } else {
                                    fill
                                },
                            ));
                        });
                    });
                    // The icon in its dark socket, glowing in the rarity's colour.
                    c.spawn((
                        Node {
                            width: Val::Px(70.0),
                            height: Val::Px(70.0),
                            margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(10.0), Val::Px(14.0)),
                            border: UiRect::all(Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BorderColor::all(Color::srgb(0.25, 0.23, 0.2)),
                        BoxShadow::new(
                            rc.with_alpha(if common { 0.15 } else { 0.5 }),
                            Val::Px(0.0),
                            Val::Px(0.0),
                            Val::Px(2.0),
                            Val::Px(16.0),
                        ),
                    ))
                    .with_children(|s| {
                        s.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            ImageNode {
                                color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                                ..default()
                            },
                            SpellIcon(card.spell),
                        ));
                    });
                    c.spawn(hud::centered_bundle(
                        f,
                        spaced(rarity_label(rarity)),
                        11.0,
                        if common { CARD_TEXT } else { rc },
                    ));
                    c.spawn(hud::centered_bundle(f, card.name, 22.0, name_color));
                    c.spawn(hud::centered_bundle(f, card.tag, 13.0, CARD_DIM));
                    c.spawn((
                        Node {
                            width: Val::Percent(70.0),
                            height: Val::Px(1.0),
                            margin: UiRect::vertical(Val::Px(6.0)),
                            ..default()
                        },
                        BackgroundGradient::from(LinearGradient::to_right(vec![
                            ColorStop::percent(edge.with_alpha(0.0), 0.0),
                            ColorStop::percent(edge.with_alpha(0.7), 50.0),
                            ColorStop::percent(edge.with_alpha(0.0), 100.0),
                        ])),
                    ));
                    if !card.desc.is_empty() {
                        c.spawn(hud::centered_bundle(f, card.desc, 13.0, CARD_TEXT));
                    }
                    // What the card adds: the number in green, what it grows after it.
                    for (value, what) in card.perks {
                        c.spawn(Node {
                            column_gap: Val::Px(5.0),
                            margin: UiRect::top(Val::Px(2.0)),
                            ..default()
                        })
                        .with_children(|l| {
                            l.spawn(hud::text_bundle(f, value, 16.0, GAIN));
                            l.spawn(hud::text_bundle(f, what, 16.0, CARD_TEXT));
                        });
                    }
                    // The card's foot, under a hairline.
                    c.spawn(Node {
                        flex_grow: 1.0,
                        ..default()
                    });
                    if !card.footer.is_empty() {
                        c.spawn((
                            Node {
                                width: Val::Percent(112.0),
                                justify_content: JustifyContent::Center,
                                padding: UiRect::vertical(Val::Px(8.0)),
                                border: UiRect::top(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.1)),
                            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.25)),
                        ))
                        .with_children(|ft| {
                            ft.spawn(hud::centered_bundle(f, card.footer, 12.0, CARD_DIM));
                        });
                    }
                    // The landing shine, a bright band that crosses once.
                    c.spawn((
                        CardShine(i),
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(-40.0),
                            bottom: Val::Px(-40.0),
                            width: Val::Px(70.0),
                            left: Val::Px(-120.0),
                            ..default()
                        },
                        BackgroundGradient::from(LinearGradient::to_right(vec![
                            ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.0), 0.0),
                            ColorStop::percent(Color::srgba(1.0, 0.97, 0.85, 0.22), 50.0),
                            ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.0), 100.0),
                        ])),
                        UiTransform::from_rotation(Rot2::degrees(18.0)),
                        bevy::ui::FocusPolicy::Pass,
                    ));
                });
                // Banish (N) under the card, for a spell's card while a banish is in hand.
                col.spawn((
                    BanishButton(i),
                    Button,
                    Interaction::default(),
                    Node {
                        padding: UiRect::axes(Val::Px(16.0), Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.22, 0.04, 0.03, 0.92)),
                    BorderColor::all(Color::srgb(0.7, 0.22, 0.15)),
                    if banishable && run.banishes > 0 {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    },
                ))
                .with_children(|b| {
                    b.spawn(hud::text_bundle(
                        f,
                        trf("Banish  ({})", &[&run.banishes.to_string()]),
                        15.0,
                        Color::srgb(0.95, 0.42, 0.32),
                    ));
                });
            });
        }
    });
}

fn ease_out_back(t: f32) -> f32 {
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

/// Deal, tilt, lift and burst: real time, since the game clock is paused under the picker.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn animate_cards(
    time: Res<Time<Real>>,
    phase: Res<State<Phase>>,
    mut cards: Query<(
        &mut Card,
        &Interaction,
        &RelativeCursorPosition,
        &mut UiTransform,
        &mut BoxShadow,
        &mut BorderColor,
        &mut Visibility,
    )>,
    mut shines: Query<(&CardShine, &mut Node)>,
    mut rays: Query<&mut UiTransform, (With<Rays>, Without<Card>)>,
    mut pending: ResMut<PendingPick>,
    mut picks: ResMut<super::sync::LocalPicks>,
    mut run: ResMut<Run>,
    mut offer: ResMut<Offer>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::LevelUp {
        pending.0 = None;
        return;
    }
    let dt = time.delta_secs().min(0.05);
    let now = time.elapsed_secs();
    for mut r in &mut rays {
        r.rotation = Rot2::radians(now * 0.12);
        r.scale = Vec2::splat(1.0 + 0.03 * (now * 1.7).sin());
    }
    let chosen = pending.0.map(|(m, _)| match m {
        Move::Pick(i) | Move::Banish(i) => i,
        Move::Reroll | Move::Skip => usize::MAX,
    });
    let banishing = offer.banishing || matches!(pending.0, Some((Move::Banish(_), _)));
    let mut shine_at = [None::<f32>; 4];
    for (mut card, interaction, cursor, mut tf, mut shadow, mut border, mut vis) in &mut cards {
        let card = &mut *card;
        card.t += dt;
        if card.t < 0.0 {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Inherited;
        let deal = (card.t / DEAL_SECS).clamp(0.0, 1.0);
        let e = ease_out_back(deal);
        let hot = *interaction != Interaction::None && chosen.is_none() && deal >= 1.0;
        let k = 1.0 - (-dt * 14.0).exp();
        let hv = card.hover;
        card.hover = hv + ((if hot { 1.0 } else { 0.0 }) - hv) * k;
        let want_tilt = if hot {
            cursor
                .normalized
                .unwrap_or_default()
                .clamp(Vec2::splat(-0.5), Vec2::splat(0.5))
        } else {
            Vec2::ZERO
        };
        let tl = card.tilt;
        card.tilt = tl + (want_tilt - tl) * k;
        let h = card.hover;
        // The pick: the chosen card swells and rises, the others drop away.
        let (pick_scale, pick_y, pick_fade) = match (chosen, card.picked) {
            (Some(c), _) if c == card.index => {
                let p = card.picked.get_or_insert(0.0);
                *p += dt;
                let u = (*p / PICK_SECS).clamp(0.0, 1.0);
                (1.0 + 0.18 * u, -30.0 * u, 0.0)
            }
            (Some(_), _) => {
                let p = card.picked.get_or_insert(0.0);
                *p += dt;
                let u = (*p / PICK_SECS).clamp(0.0, 1.0);
                (1.0 - 0.15 * u, 140.0 * u * u, u)
            }
            _ => (1.0, 0.0, 0.0),
        };
        // A perspective tilt, faked: the card turns about the cursor's side and leans with it.
        let lean = card.tilt.x * 7.0;
        let squash = 1.0 - card.tilt.x.abs() * 0.08;
        let bob = (now * 2.2 + card.index as f32 * 1.3).sin() * 1.5 * (1.0 - h);
        tf.translation = Val2::px(
            0.0,
            (1.0 - e) * 260.0 - 18.0 * h + card.tilt.y * 8.0 + bob + pick_y,
        );
        let s = (0.6 + 0.4 * e) * (1.0 + 0.06 * h) * pick_scale;
        tf.scale = Vec2::new(s * squash, s * (1.0 + card.tilt.y.abs() * 0.02));
        tf.rotation = Rot2::degrees((1.0 - e) * -14.0 + lean);
        let rc = if banishing {
            BANISH
        } else {
            rarity_color(card.rarity)
        };
        let edge = if banishing && h > 0.5 {
            BANISH
        } else {
            rarity_color(card.rarity)
        };
        if border.top != edge {
            *border = BorderColor::all(edge);
        }
        if let Some(sh) = shadow.0.first_mut() {
            sh.color = rc.with_alpha((0.25 + 0.6 * h) * (1.0 - pick_fade));
            sh.blur_radius = Val::Px(18.0 + 22.0 * h);
            sh.spread_radius = Val::Px(2.0 + 4.0 * h);
            sh.y_offset = Val::Px(6.0 + 10.0 * h);
        }
        if pick_fade >= 1.0 {
            *vis = Visibility::Hidden;
        }
        if card.index < shine_at.len() {
            shine_at[card.index] = Some(card.t);
        }
    }
    for (shine, mut node) in &mut shines {
        let Some(t) = shine_at.get(shine.0).copied().flatten() else {
            continue;
        };
        let u = ((t - DEAL_SECS * 0.6) / 0.5).clamp(0.0, 1.0);
        node.left = Val::Px(-120.0 + u * (CARD_W + 200.0));
    }
    if let Some((m, t)) = pending.0.as_mut() {
        *t += dt;
        if *t >= PICK_SECS {
            let m = *m;
            pending.0 = None;
            hud::make_move(m, &mut run, &mut offer, &mut next, &mut picks);
        }
    }
}

/// What the buttons under the cards do; Banish is the per-card buttons' and B's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tool {
    Reroll,
    Banish,
    Skip,
}

impl Tool {
    /// The row's buttons, in order.
    const ROW: [Tool; 2] = [Tool::Skip, Tool::Reroll];
}

#[derive(Component)]
pub(super) struct ToolButton(Tool);

#[derive(Component)]
pub(super) struct ToolLabel(Tool);

const TOOL_BG: Color = Color::srgba(0.16, 0.06, 0.04, 0.94);
const TOOL_HOT: Color = Color::srgba(0.3, 0.12, 0.06, 0.96);
const TOOL_EDGE: Color = Color::srgb(0.85, 0.62, 0.3);

/// The row under the cards: SPELLS and the hero's spell slots, then Skip (+ gold) and Reroll N
/// with its key.
pub(super) fn spawn_tools(root: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    root.spawn(Node {
        column_gap: Val::Px(14.0),
        align_items: AlignItems::Center,
        margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(10.0), Val::Px(6.0)),
        ..default()
    })
    .with_children(|row| {
        row.spawn(hud::text_bundle(font, spaced(tr("Spells")), 13.0, CARD_DIM));
        row.spawn((
            SpellStrip,
            Node {
                column_gap: Val::Px(5.0),
                margin: UiRect::right(Val::Px(18.0)),
                ..default()
            },
        ));
        for tool in Tool::ROW {
            row.spawn((
                ToolButton(tool),
                Button,
                Interaction::default(),
                Node {
                    padding: UiRect::axes(Val::Px(18.0), Val::Px(7.0)),
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(5.0)),
                    ..default()
                },
                BackgroundColor(TOOL_BG),
                BorderColor::all(TOOL_EDGE.with_alpha(0.75)),
            ))
            .with_children(|b| {
                b.spawn((
                    ToolLabel(tool),
                    hud::text_bundle(font, "", 20.0, Color::srgb(1.0, 0.85, 0.55)),
                ));
                // Skip's gold, small; Reroll's key in its cap.
                match tool {
                    Tool::Skip => {
                        b.spawn(hud::text_bundle(
                            font,
                            trf("+{} gold", &[&data::SKIP_GOLD.to_string()]),
                            14.0,
                            Color::srgb(0.95, 0.8, 0.5),
                        ));
                    }
                    _ => {
                        b.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(5.0), Val::Px(0.0)),
                                border: UiRect::all(Val::Px(1.0)),
                                border_radius: BorderRadius::all(Val::Px(3.0)),
                                ..default()
                            },
                            BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.4)),
                        ))
                        .with_children(|k| {
                            k.spawn(hud::text_bundle(font, "R", 12.0, CARD_DIM));
                        });
                    }
                }
            });
        }
    });
}

/// Reroll and Skip move at once; Banish arms itself, for the next card chosen.
fn use_tool(
    tool: Tool,
    run: &mut Run,
    offer: &mut Offer,
    next: &mut NextState<Phase>,
    picks: &mut super::sync::LocalPicks,
) {
    // A blessing is only ever picked.
    if offer.blessing {
        return;
    }
    match tool {
        Tool::Reroll => {
            hud::make_move(Move::Reroll, run, offer, next, picks);
        }
        Tool::Skip => {
            hud::make_move(Move::Skip, run, offer, next, picks);
        }
        Tool::Banish => offer.banishing = !offer.banishing && run.banishes > 0,
    }
}

/// R rerolls, B arms (or disarms) a banish, X skips: while no card is on its way out.
#[allow(clippy::too_many_arguments)]
pub(super) fn card_keys(
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    pending: Res<PendingPick>,
    mut run: ResMut<Run>,
    mut offer: ResMut<Offer>,
    mut next: ResMut<NextState<Phase>>,
    mut picks: ResMut<super::sync::LocalPicks>,
) {
    if *phase.get() != Phase::LevelUp || pending.0.is_some() {
        return;
    }
    let tool = if keys.just_pressed(KeyCode::KeyR) {
        Some(Tool::Reroll)
    } else if keys.just_pressed(KeyCode::KeyB) {
        Some(Tool::Banish)
    } else if keys.just_pressed(KeyCode::KeyX) {
        Some(Tool::Skip)
    } else if keys.just_pressed(KeyCode::Escape) && offer.banishing {
        offer.banishing = false;
        None
    } else {
        None
    };
    if let Some(tool) = tool {
        use_tool(tool, &mut run, &mut offer, &mut next, &mut picks);
    }
}

/// The row's buttons: Skip always there, Reroll with what is left of it and greyed at none; lit
/// under the cursor; a click uses it. The Banish buttons under the cards start a card's banish.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn card_buttons(
    phase: Res<State<Phase>>,
    mut pending: ResMut<PendingPick>,
    mut run: ResMut<Run>,
    mut offer: ResMut<Offer>,
    mut next: ResMut<NextState<Phase>>,
    mut picks: ResMut<super::sync::LocalPicks>,
    mut buttons: Query<(
        &ToolButton,
        &Interaction,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut labels: Query<(&ToolLabel, &mut Text, &mut TextColor)>,
    banish: Query<(&BanishButton, &Interaction), Changed<Interaction>>,
    cards: Query<&Card>,
) {
    if *phase.get() != Phase::LevelUp {
        return;
    }
    let mut clicked = None;
    for (b, interaction, mut bg, mut border) in &mut buttons {
        let has = !offer.blessing && (b.0 == Tool::Skip || run.rerolls > 0);
        let want = if has && *interaction != Interaction::None {
            TOOL_HOT
        } else {
            TOOL_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
        let edge = if has {
            TOOL_EDGE.with_alpha(0.75)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.12)
        };
        if border.top != edge {
            *border = BorderColor::all(edge);
        }
        if *interaction == Interaction::Pressed && has {
            clicked = Some(b.0);
        }
    }
    for (l, mut text, mut color) in &mut labels {
        let (want, on) = match l.0 {
            Tool::Skip => (tr("Skip").to_string(), !offer.blessing),
            _ => (
                format!("{} {}", tr("Reroll"), run.rerolls),
                run.rerolls > 0 && !offer.blessing,
            ),
        };
        if text.0 != want {
            text.0 = want;
        }
        let c = if on {
            Color::srgb(1.0, 0.85, 0.55)
        } else {
            hud::DIM_GREY
        };
        if color.0 != c {
            color.0 = c;
        }
    }
    if pending.0.is_some() {
        return;
    }
    if let Some(tool) = clicked {
        use_tool(tool, &mut run, &mut offer, &mut next, &mut picks);
        return;
    }
    // A card's own Banish: it goes the way a pick does, once it has landed.
    let dealt = |i: usize| cards.iter().any(|c| c.index == i && c.t >= DEAL_SECS);
    if let Some((b, _)) = banish
        .iter()
        .find(|(b, i)| **i == Interaction::Pressed && dealt(b.0))
    {
        if run.banishes > 0 && !offer.blessing && b.0 < offer.len() {
            pending.0 = Some((Move::Banish(b.0), 0.0));
        }
    }
}

/// A click on a card, or 1 to 4: start its burst; the pick (or the banish, when armed) is made
/// when it finishes.
pub(super) fn choose_card(
    phase: Res<State<Phase>>,
    keys: Res<ButtonInput<KeyCode>>,
    cards: Query<(&Card, &Interaction), Changed<Interaction>>,
    offer: Res<Offer>,
    mut pending: ResMut<PendingPick>,
) {
    if *phase.get() != Phase::LevelUp || pending.0.is_some() {
        return;
    }
    let mut pick = cards
        .iter()
        .find(|(c, i)| **i == Interaction::Pressed && c.t >= DEAL_SECS)
        .map(|(c, _)| c.index);
    for (i, key) in [
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
    ]
    .into_iter()
    .enumerate()
    {
        if keys.just_pressed(key) {
            pick = Some(i);
        }
    }
    if let Some(i) = pick.filter(|&i| i < offer.len()) {
        let m = if offer.banishing {
            Move::Banish(i)
        } else {
            Move::Pick(i)
        };
        pending.0 = Some((m, 0.0));
    }
}
