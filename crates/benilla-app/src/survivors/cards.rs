//! The level-up picker, as a survivors game deals it: three tall cards that flip up out of the
//! deck one after another, tilt toward the cursor, lift under it and burst on the pick. Each card
//! wears the rarity it was drawn at (`offer.rs` deals it) in WoW's item-quality colours, and its
//! numbers are the rank's at that rarity's worth. Under the row, Reroll (R), Banish (B, then a
//! card) and Skip (X), each with what is left of it.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use super::data::CardRarity;
use super::hud::{self, HudFont, Move, Offer, SpellIcon};
use super::lang::tr;
use super::{Phase, Run};

/// WoW's item-quality colours (`ITEM_QUALITY_COLORS`).
pub(super) const COMMON: Color = Color::srgb(1.0, 1.0, 1.0);
pub(super) const UNCOMMON: Color = Color::srgb(0.118, 1.0, 0.0);
pub(super) const RARE: Color = Color::srgb(0.0, 0.439, 0.867);
pub(super) const EPIC: Color = Color::srgb(0.639, 0.208, 0.933);
pub(super) const LEGENDARY: Color = Color::srgb(1.0, 0.502, 0.0);

const CARD_W: f32 = 236.0;
const CARD_H: f32 = 330.0;
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

/// The cards, rebuilt for each offer.
pub(super) fn deal_cards(
    mut commands: Commands,
    offer: Res<Offer>,
    run: Res<Run>,
    spells: Option<Res<crate::ui_action::Spells>>,
    font: Res<HudFont>,
    rows: Query<Entity, With<hud::OfferRow>>,
) {
    if !offer.is_changed() {
        return;
    }
    let Ok(row) = rows.single() else { return };
    commands.entity(row).despawn_related::<Children>();
    let f = &font.0;
    commands.entity(row).with_children(|row| {
        for (i, (dealt, choice)) in offer.cards().iter().enumerate() {
            let rarity = dealt.rarity;
            let card = hud::describe_card(*choice, rarity, &run, spells.as_deref());
            let rc = rarity_color(rarity);
            row.spawn((
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
                        Val::Px(16.0),
                        Val::Px(16.0),
                        Val::Px(18.0),
                        Val::Px(14.0),
                    ),
                    row_gap: Val::Px(6.0),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(10.0)),
                    ..default()
                },
                // A dark card face lit from the top in the rarity's colour.
                BackgroundGradient::from(LinearGradient::to_bottom(vec![
                    ColorStop::percent(rc.mix(&Color::BLACK, 0.55).with_alpha(0.98), 0.0),
                    ColorStop::percent(Color::srgba(0.05, 0.04, 0.035, 0.97), 42.0),
                    ColorStop::percent(Color::srgba(0.02, 0.015, 0.01, 0.98), 100.0),
                ])),
                BorderColor::all(rc),
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
                            ColorStop::percent(rc.with_alpha(0.45), 0.0),
                            ColorStop::percent(rc.with_alpha(0.0), 100.0),
                        ],
                    )),
                    bevy::ui::FocusPolicy::Pass,
                ));
                // The key and the rarity on one line along the top.
                c.spawn(Node {
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|top| {
                    top.spawn(hud::text_bundle(
                        f,
                        format!("{}", i + 1),
                        15.0,
                        hud::DIM_GREY,
                    ));
                    top.spawn(hud::text_bundle(f, rarity_label(rarity), 13.0, rc));
                });
                // The icon in a gold-rimmed socket.
                c.spawn((
                    Node {
                        width: Val::Px(74.0),
                        height: Val::Px(74.0),
                        margin: UiRect::vertical(Val::Px(6.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(6.0)),
                        ..default()
                    },
                    BorderColor::all(rc.mix(&hud::GOLD_TONE, 0.4)),
                    BoxShadow::new(
                        rc.with_alpha(0.55),
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(2.0),
                        Val::Px(14.0),
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
                    card.name,
                    21.0,
                    rc.mix(&Color::WHITE, 0.25),
                ));
                c.spawn(hud::centered_bundle(f, card.tag, 14.0, card.tag_color));
                // The pips: rank now, and the one this pick fills.
                if let Some((have, max)) = card.ranks {
                    c.spawn(Node {
                        column_gap: Val::Px(4.0),
                        margin: UiRect::vertical(Val::Px(2.0)),
                        ..default()
                    })
                    .with_children(|pips| {
                        for r in 0..max {
                            let color = if r < have {
                                hud::GOLD_TONE
                            } else if r == have {
                                rc
                            } else {
                                Color::srgba(1.0, 1.0, 1.0, 0.12)
                            };
                            pips.spawn((
                                Node {
                                    width: Val::Px(14.0),
                                    height: Val::Px(6.0),
                                    border_radius: BorderRadius::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(color),
                            ));
                        }
                    });
                }
                c.spawn((
                    Node {
                        width: Val::Percent(80.0),
                        height: Val::Px(1.0),
                        margin: UiRect::vertical(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundGradient::from(LinearGradient::to_right(vec![
                        ColorStop::percent(rc.with_alpha(0.0), 0.0),
                        ColorStop::percent(rc.with_alpha(0.8), 50.0),
                        ColorStop::percent(rc.with_alpha(0.0), 100.0),
                    ])),
                ));
                c.spawn(hud::centered_bundle(
                    f,
                    card.desc,
                    14.0,
                    Color::srgb(0.92, 0.9, 0.84),
                ));
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
                        ColorStop::percent(Color::srgba(1.0, 0.97, 0.85, 0.28), 50.0),
                        ColorStop::percent(Color::srgba(1.0, 1.0, 1.0, 0.0), 100.0),
                    ])),
                    UiTransform::from_rotation(Rot2::degrees(18.0)),
                    bevy::ui::FocusPolicy::Pass,
                ));
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

/// What the buttons under the cards do.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Tool {
    Reroll,
    Banish,
    Skip,
}

impl Tool {
    const ALL: [Tool; 3] = [Tool::Reroll, Tool::Banish, Tool::Skip];

    fn label(self) -> &'static str {
        tr(match self {
            Tool::Reroll => "Reroll",
            Tool::Banish => "Banish",
            Tool::Skip => "Skip",
        })
    }

    fn key(self) -> &'static str {
        match self {
            Tool::Reroll => "R",
            Tool::Banish => "B",
            Tool::Skip => "X",
        }
    }

    fn held(self, run: &Run) -> u8 {
        match self {
            Tool::Reroll => run.rerolls,
            Tool::Banish => run.banishes,
            Tool::Skip => run.skips,
        }
    }
}

#[derive(Component)]
pub(super) struct ToolButton(Tool);

#[derive(Component)]
pub(super) struct ToolLabel(Tool);

const TOOL_BG: Color = Color::srgba(0.08, 0.06, 0.04, 0.92);
const TOOL_HOT: Color = Color::srgba(0.22, 0.16, 0.08, 0.95);

/// The row of Reroll, Banish and Skip under the cards.
pub(super) fn spawn_tools(root: &mut ChildSpawnerCommands, font: &Handle<Font>) {
    root.spawn(Node {
        column_gap: Val::Px(14.0),
        margin: UiRect::bottom(Val::Px(10.0)),
        ..default()
    })
    .with_children(|row| {
        for tool in Tool::ALL {
            row.spawn((
                ToolButton(tool),
                Button,
                Interaction::default(),
                Node {
                    min_width: Val::Px(150.0),
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
                    justify_content: JustifyContent::Center,
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(TOOL_BG),
                BorderColor::all(hud::GOLD_TONE.with_alpha(0.6)),
            ))
            .with_children(|b| {
                b.spawn((
                    ToolLabel(tool),
                    hud::text_bundle(font, tool.label(), 15.0, Color::WHITE),
                ));
            });
        }
    });
}

/// The buttons under the cards: what is left of each on its label, greyed when none is, lit
/// under the cursor and while a banish is armed; a click uses it.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn card_buttons(
    phase: Res<State<Phase>>,
    pending: Res<PendingPick>,
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
) {
    if *phase.get() != Phase::LevelUp {
        return;
    }
    let mut clicked = None;
    for (b, interaction, mut bg, mut border) in &mut buttons {
        let armed = b.0 == Tool::Banish && offer.banishing;
        let has = b.0.held(&run) > 0 && !offer.blessing;
        let want = if armed || (has && *interaction != Interaction::None) {
            TOOL_HOT
        } else {
            TOOL_BG
        };
        if bg.0 != want {
            bg.0 = want;
        }
        let edge = if armed {
            BANISH
        } else if has {
            hud::GOLD_TONE.with_alpha(0.6)
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
        let n = l.0.held(&run);
        let want = format!("{} ({})  ·  {}", l.0.label(), l.0.key(), n);
        if text.0 != want {
            text.0 = want;
        }
        let c = if n > 0 { Color::WHITE } else { hud::DIM_GREY };
        if color.0 != c {
            color.0 = c;
        }
    }
    if let Some(tool) = clicked.filter(|_| pending.0.is_none()) {
        use_tool(tool, &mut run, &mut offer, &mut next, &mut picks);
    }
}

/// A click on a card, or 1/2/3: start its burst; the pick (or the banish, when armed) is made
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
    for (i, key) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
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
