//! The mode's screens, in bevy_ui since FrameXML is off: the class and hero pick, the in-run HUD,
//! the level-up cards, pause, and the results.

use std::collections::HashMap;

use benilla_ui::script::nameplate;
use benilla_world::view::WorldCamera;
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::ui_material::{MaterialNode, UiMaterial};
use bevy::window::PrimaryWindow;

use super::data::{self, AbilityDef, Effect, Kind, PassiveDef, Stat};
use super::hero::{self, Hero};
use super::minimap::{self, MiniMap};
use super::offer::Dealt;
use super::{Owned, Phase, Rng, Run, Selection, Stats, Vitals};

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
/// The mobility button's edge, an action button's 36 units.
const MOBILITY_SIZE: f32 = super::frames::SLOT;
const EDGE: Color = Color::srgb(0.55, 0.45, 0.25);
const BUTTON: Color = Color::srgba(0.18, 0.06, 0.04, 0.95);
const BUTTON_HOT: Color = Color::srgba(0.36, 0.12, 0.06, 0.98);
const DIM: Color = Color::srgb(0.6, 0.6, 0.6);
pub(super) const DIM_GREY: Color = DIM;
pub(super) const GOLD_TONE: Color = GOLD;

#[derive(Component)]
pub(super) struct MenuRoot;
#[derive(Component)]
pub(super) struct HudRoot;
#[derive(Component)]
pub(super) struct LevelUpRoot;
#[derive(Component)]
pub(super) struct PauseRoot;
#[derive(Component)]
pub(super) struct OverRoot;
#[derive(Component)]
pub(super) struct XpBar;
#[derive(Component)]
pub(super) struct XpFill;
#[derive(Component)]
pub(super) struct XpText;
#[derive(Component)]
pub(super) struct LevelText;
#[derive(Component)]
pub(super) struct TimerText;
#[derive(Component)]
pub(super) struct KillText;
#[derive(Component)]
pub(super) struct HpFrame;
#[derive(Component)]
pub(super) struct HpBar;
#[derive(Component)]
pub(super) struct HpLevelSeat;
#[derive(Component)]
pub(super) struct HpLevel;
#[derive(Component)]
pub(super) struct HpFill;
#[derive(Component)]
pub(super) struct ShieldFill;
#[derive(Component)]
pub(super) struct BannerText;
#[derive(Component)]
pub(super) struct AbilityRow;
#[derive(Component)]
pub(super) struct PassiveRow;
/// The run's items, each with its stacks, under the passives.
#[derive(Component)]
pub(super) struct ItemRow;
#[derive(Component)]
pub(super) struct OfferRow;
/// The line under LEVEL UP: which level this is.
#[derive(Component)]
pub(super) struct LevelUpSub;
/// The picker's title: a level up, or a shrine's blessing.
#[derive(Component)]
pub(super) struct LevelUpTitle;
/// The gold badge over the title: the level reached.
#[derive(Component)]
pub(super) struct LevelBadge;

/// A radial cooldown sweep over one ability icon: `sweep` is the remaining fraction
/// (1.0 = just cast, 0.0 = ready), drawn by `ui_sweep.wgsl`.
#[derive(Asset, TypePath, AsBindGroup, Clone)]
pub(super) struct SweepUiMaterial {
    #[uniform(0)]
    pub(super) sweep: f32,
}

impl UiMaterial for SweepUiMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://benilla_app/shaders/ui_sweep.wgsl".into()
    }
}

/// The cooldown overlay on one ability slot, driven every frame.
#[derive(Component)]
pub(super) struct AbilitySweep {
    handle: Handle<SweepUiMaterial>,
    index: usize,
}

/// The class mobility's action button, bottom centre over the XP bar; shown once it is learned.
#[derive(Component)]
pub(super) struct MobilitySlot;
#[derive(Component)]
pub(super) struct MobilityIcon;
#[derive(Component)]
pub(super) struct MobilitySweep(Handle<SweepUiMaterial>);
#[derive(Component)]
pub(super) struct OverTitle;
#[derive(Component)]
pub(super) struct OverStats;
#[derive(Component)]
pub(super) struct OverLine;
#[derive(Component)]
pub(super) struct OverStat(usize);

/// An icon slot filled from the spell's `SpellIcon` once the client data is up.
#[derive(Component)]
pub(super) struct SpellIcon(pub(super) u32);
#[derive(Component)]
pub(super) struct IconDone;

/// A FrameXML `Texture` region: a client art file, its `TexCoords` (left, right, top, bottom) and
/// vertex colour, drawn once the archives are readable.
#[derive(Component)]
pub(super) struct UiArt {
    path: &'static str,
    coords: [f32; 4],
    tint: Color,
}

impl UiArt {
    pub(super) fn crop(path: &'static str, coords: [f32; 4]) -> Self {
        Self {
            path,
            coords,
            tint: Color::WHITE,
        }
    }

    pub(super) fn path(&self) -> &'static str {
        self.path
    }

    pub(super) fn tinted(path: &'static str, tint: Color) -> Self {
        Self {
            path,
            coords: [0.0, 1.0, 0.0, 1.0],
            tint,
        }
    }
}

#[derive(Component)]
pub(super) struct ArtDone;

/// What an icon slot's tooltip describes: an index into the run's abilities or passives.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum TipFor {
    Ability(usize),
    Passive(usize),
    /// An index into the run's items.
    RunItem(usize),
    Mobility,
    /// An index into the merchant's wares.
    Ware(usize),
}

/// The one `GameTooltip`, and the column its lines go in.
#[derive(Component)]
pub(super) struct GameTooltip;
#[derive(Component)]
pub(super) struct TipBody;

#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum UiAction {
    Resume,
    MainMenu,
    Quit,
}

/// The colour a button returns to when not hovered.
#[derive(Component)]
pub(super) struct BaseColor(Color);

#[derive(Clone, Copy)]
pub(super) enum Choice {
    NewAbility(&'static AbilityDef),
    Upgrade(usize),
    NewPassive(&'static PassiveDef),
    UpPassive(usize),
    Heal,
    /// A Shrine of Power's blessing, by its index in the hero's shrine offer.
    Boon(usize),
    /// A chest's run item, by its index in the hero's chest offer.
    ChestItem(usize),
}

/// The line under the cards: how to pick, or how to banish.
#[derive(Component)]
pub(super) struct RerollHint;

/// The lines around the cards: the level's badge, a level up's or a blessing's title and its
/// line, and how to pick, or that the next card chosen is banished.
#[allow(clippy::type_complexity)]
pub(super) fn reroll_hint(
    offer: Res<Offer>,
    run: Res<Run>,
    mut texts: ParamSet<(
        Query<&mut Text, With<RerollHint>>,
        Query<&mut Text, With<LevelUpTitle>>,
        Query<&mut Text, With<LevelUpSub>>,
        Query<&mut Text, With<LevelBadge>>,
    )>,
) {
    use super::lang::{tr, trf};
    let badge = if offer.chest {
        tr("CHEST").to_string()
    } else if offer.blessing {
        tr("SHRINE").to_string()
    } else {
        trf("LEVEL {}", &[&run.level.to_string()])
    };
    let (title, sub, hint) = if offer.chest {
        (
            tr("Chest!"),
            tr("Choose an item for the rest of the run."),
            tr("Click an item, or press 1, 2 or 3"),
        )
    } else if offer.blessing {
        (
            tr("Blessing!"),
            tr("Choose a blessing for the rest of the run."),
            tr("Click a blessing, or press 1, 2 or 3"),
        )
    } else if offer.banishing {
        (
            tr("Level Up!"),
            tr("Choose a spell. Its rarity decides how strong it is."),
            tr("Banish: click a card, or press 1 to 4  ·  B again to cancel"),
        )
    } else {
        (
            tr("Level Up!"),
            tr("Choose a spell. Its rarity decides how strong it is."),
            tr("Click a card, or press 1 to 4"),
        )
    };
    let set = |t: &mut Text, want: &str| {
        if t.0 != want {
            t.0 = want.to_string();
        }
    };
    for mut t in &mut texts.p0() {
        set(&mut t, hint);
    }
    for mut t in &mut texts.p1() {
        set(&mut t, title);
    }
    for mut t in &mut texts.p2() {
        set(&mut t, sub);
    }
    for mut t in &mut texts.p3() {
        set(&mut t, &badge);
    }
}

/// The level-up cards on show: the local hero's offer as the view has it, each as the HUD names
/// it. The simulation deals the offer (`offer.rs`); this only follows it.
#[derive(Resource, Default)]
pub(super) struct Offer {
    cards: Vec<(Dealt, Choice)>,
    /// What the cards were last built from: the offer, the moves made, and whether they are
    /// a chest's items or blessings.
    shown: Option<(Vec<Dealt>, u32, bool, bool)>,
    /// The next card chosen is banished, not picked.
    pub(super) banishing: bool,
    /// The cards are only picked, never rerolled, banished or skipped: a Shrine of Power's
    /// blessings, or a chest's items ([`Self::chest`]).
    pub(super) blessing: bool,
    /// The cards are an opened chest's run items.
    pub(super) chest: bool,
}

impl Offer {
    pub(super) fn len(&self) -> usize {
        self.cards.len()
    }

    pub(super) fn cards(&self) -> &[(Dealt, Choice)] {
        &self.cards
    }
}

/// A dealt card as the HUD names it, by index into the hero's abilities and passives.
pub(super) fn view_choice(d: &Dealt, v: &super::Vitals) -> Option<Choice> {
    use survivors_net::game::Choice as W;
    Some(match &d.choice {
        W::NewAbility(k) => Choice::NewAbility(data::ABILITIES.iter().find(|a| a.key == k)?),
        W::Upgrade(k) => Choice::Upgrade(v.abilities.iter().position(|o| o.def.key == k)?),
        W::NewPassive(k) => Choice::NewPassive(data::PASSIVES.iter().find(|p| p.key == k)?),
        W::UpPassive(k) => Choice::UpPassive(v.passives.iter().position(|(p, ..)| p.key == k)?),
        W::Heal => Choice::Heal,
    })
}

/// The cards follow the local hero's offer: rebuilt when it, or the moves made, change.
pub(super) fn follow_offer(phase: Res<State<Phase>>, run: Res<Run>, mut offer: ResMut<Offer>) {
    if *phase.get() != Phase::LevelUp {
        if offer.shown.is_some() || offer.banishing || !offer.cards.is_empty() {
            *offer = Offer::default();
        }
        return;
    }
    // A chest's items go first, then a blessing waiting: their cards wear their rarity as a card
    // does.
    let chest = !run.chest_offer.is_empty();
    let blessing = chest || !run.shrine_offer.is_empty();
    let cards: Vec<(Dealt, Choice)> = if chest {
        run.chest_offer
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let d = Dealt {
                    choice: survivors_net::game::Choice::Heal,
                    rarity: item.rarity,
                    perks: Vec::new(),
                };
                (d, Choice::ChestItem(i))
            })
            .collect()
    } else if blessing {
        run.shrine_offer
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let d = Dealt {
                    choice: survivors_net::game::Choice::Heal,
                    rarity: b.rarity,
                    perks: Vec::new(),
                };
                (d, Choice::Boon(i))
            })
            .collect()
    } else {
        run.offer
            .iter()
            .filter_map(|d| view_choice(d, &run).map(|c| (d.clone(), c)))
            .collect()
    };
    let key = (
        cards.iter().map(|(d, _)| d.clone()).collect(),
        run.picks,
        blessing,
        chest,
    );
    if offer.shown.as_ref() == Some(&key) {
        return;
    }
    offer.cards = cards;
    offer.blessing = blessing;
    offer.chest = chest;
    if blessing {
        offer.banishing = false;
    }
    offer.shown = Some(key);
}

/// A move at the cards, from the HUD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Move {
    Pick(usize),
    Banish(usize),
    Reroll,
    Skip,
}

/// Make a move at the cards: sent to the run (solo, host) or the host (guest), and shown at once.
/// False when the hero has nothing to make it with.
pub(super) fn make_move(
    m: Move,
    run: &mut Run,
    offer: &mut Offer,
    next: &mut NextState<Phase>,
    picks: &mut super::sync::LocalPicks,
) -> bool {
    use survivors_net::game::CardAction;
    let done = |run: &Run| {
        run.pending_levels == 0 && run.shrine_offer.is_empty() && run.chest_offer.is_empty()
    };
    if offer.chest {
        let Move::Pick(i) = m else { return false };
        let Some(Choice::ChestItem(k)) = offer.cards.get(i).map(|(_, c)| *c) else {
            return false;
        };
        let Some(item) = run.chest_offer.get(k).copied() else {
            return false;
        };
        if !super::treasure::take_chest_item(&mut run.me, item.key) {
            return false;
        }
        picks.2.push(item.key.to_string());
        if done(run) {
            next.set(Phase::Playing);
        }
        return true;
    }
    if offer.blessing {
        let Move::Pick(i) = m else { return false };
        let Some(Choice::Boon(k)) = offer.cards.get(i).map(|(_, c)| *c) else {
            return false;
        };
        let Some(b) = run.shrine_offer.get(k) else {
            return false;
        };
        let wire = data::bonus_to_wire(&b.bonus);
        if !super::treasure::take_boon(&mut run.me, wire) {
            return false;
        }
        picks.1.push(wire);
        if done(run) {
            next.set(Phase::Playing);
        }
        return true;
    }
    let card = |i: usize| offer.cards.get(i).map(|(d, _)| d.to_wire());
    let action = match m {
        Move::Pick(i) | Move::Banish(i) => {
            let Some(c) = card(i) else { return false };
            if m == Move::Pick(i) {
                CardAction::Pick(c)
            } else {
                CardAction::Banish(c)
            }
        }
        Move::Reroll => CardAction::Reroll,
        Move::Skip => CardAction::Skip,
    };
    // The view takes it now. A pick or a skip lands as the run will land it; a reroll or a
    // banish waits for the deal it brings, from the run this frame or the host a round trip on.
    let ok = match &action {
        CardAction::Pick(_) | CardAction::Skip => {
            super::offer::apply_action(&mut run.me, &action, &mut Rng::seeded(0))
        }
        CardAction::Reroll | CardAction::Banish(_) => {
            let held = if matches!(action, CardAction::Reroll) {
                &mut run.me.rerolls
            } else {
                &mut run.me.banishes
            };
            if *held == 0 || run.me.offer.is_empty() {
                false
            } else {
                *held -= 1;
                run.me.offer.clear();
                run.me.picks += 1;
                true
            }
        }
    };
    if !ok {
        return false;
    }
    offer.banishing = false;
    picks.0.push(action);
    if done(run) {
        next.set(Phase::Playing);
    }
    true
}

#[derive(Resource)]
pub(super) struct HudFont(pub(super) Handle<Font>);

pub(super) fn rgb(hex: u32) -> Color {
    Color::srgb_u8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

fn text(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(1.5),
            color: Color::BLACK,
        },
    )
}

pub(super) fn text_bundle(
    font: &Handle<Font>,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    text(font, s, size, color)
}

pub(super) fn centered_bundle(
    font: &Handle<Font>,
    s: impl Into<String>,
    size: f32,
    color: Color,
) -> impl Bundle {
    centered(font, s, size, color)
}

fn centered(font: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        text(font, s, size, color),
        TextLayout::new_with_justify(Justify::Center),
    )
}

fn button(action: UiAction, base: Color) -> impl Bundle {
    edged_button(action, base, EDGE)
}

fn edged_button(action: UiAction, base: Color, edge: Color) -> impl Bundle {
    (
        Button,
        action,
        BaseColor(base),
        BackgroundColor(base),
        BorderColor::all(edge),
    )
}

fn icon(spell: u32, size: f32) -> impl Bundle {
    (
        Node {
            width: Val::Px(size),
            height: Val::Px(size),
            ..default()
        },
        ImageNode {
            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
            ..default()
        },
        SpellIcon(spell),
    )
}

pub(super) fn spell_name(
    spells: Option<&crate::ui_action::Spells>,
    spell: u32,
    fallback: &str,
) -> String {
    spells
        .and_then(|s| s.catalog.get(spell))
        .map(|d| d.name.clone())
        .unwrap_or_else(|| fallback.replace('_', " "))
}

pub(super) fn setup_hud(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut sweeps_init: ResMut<Assets<SweepUiMaterial>>,
) {
    let font = crate::char_select::wow_font(&asset_server);
    let number_font: Handle<Font> = asset_server.load("mpq://Fonts/ARIALN.ttf");
    commands.insert_resource(HudFont(font.clone()));
    let f = &font;
    let map = MiniMap::new(&mut images);

    // --- The in-run HUD. ---
    commands
        .spawn((
            HudRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            GlobalZIndex(400),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            // `MainMenuBar` under it all, its art, gryphons and twelve slots (`frames.rs`).
            super::frames::spawn_action_bar(root);
            // `MainMenuExpBar`, 1024×13 at the bottom centre, stacked on the action bar.
            root.spawn((
                XpBar,
                Interaction::default(),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    bottom: Val::Px(super::frames::BAR_TALL),
                    margin: UiRect::left(Val::Px(-512.0)),
                    width: Val::Px(1024.0),
                    height: Val::Px(13.0),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    XpFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    ImageNode {
                        color: Color::NONE,
                        ..default()
                    },
                    UiArt::tinted(
                        "Interface\\TargetingFrame\\UI-StatusBar",
                        Color::srgb(0.58, 0.0, 0.55),
                    ),
                ));
                // `MainMenuXPBarTexture0`..`3`: the segment rules, 256×10 on the bar's top.
                for (i, top) in [0.792_968_75, 0.542_968_75, 0.292_968_75, 0.042_968_75]
                    .into_iter()
                    .enumerate()
                {
                    bar.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(256.0 * i as f32),
                            top: Val::Px(0.0),
                            width: Val::Px(256.0),
                            height: Val::Px(10.0),
                            ..default()
                        },
                        ImageNode {
                            color: Color::NONE,
                            ..default()
                        },
                        UiArt::crop(
                            "Interface\\MainMenuBar\\UI-MainMenuBar-Dwarf",
                            [0.0, 1.0, top, top + 0.039_062_5],
                        ),
                    ));
                }
                // `MainMenuBarExpText`, `TextStatusBarText` centred one unit up, shown on hover.
                bar.spawn(Node {
                    position_type: PositionType::Absolute,
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    top: Val::Px(-1.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|n| {
                    n.spawn((
                        XpText,
                        Text::new(""),
                        TextFont {
                            font: number_font.clone(),
                            font_size: 14.0,
                            ..default()
                        },
                        TextColor(Color::WHITE),
                        TextShadow {
                            offset: Vec2::new(1.0, 1.0),
                            color: Color::BLACK,
                        },
                        TextLayout::new(Justify::Center, LineBreak::NoWrap),
                        Visibility::Hidden,
                    ));
                });
            });
            minimap::spawn_cluster(root, f, &map);
            // The spells, on the action bar's first slots, as `ActionButton1`.. sit.
            root.spawn((
                AbilityRow,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    bottom: Val::Px(super::frames::SLOT_BOTTOM),
                    margin: UiRect::left(Val::Px(-512.0 + super::frames::SLOT_LEFT)),
                    column_gap: Val::Px(super::frames::SLOT_STEP - super::frames::SLOT),
                    ..default()
                },
            ));
            // The mobility button on the bar's last slot, its key in the corner as
            // `ActionButtonTemplate`'s `HotKey`.
            let last = super::frames::SLOT_LEFT + super::frames::SLOT_STEP * 11.0;
            root.spawn((
                MobilitySlot,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Percent(50.0),
                    bottom: Val::Px(super::frames::SLOT_BOTTOM),
                    margin: UiRect::left(Val::Px(-512.0 + last)),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|col| {
                col.spawn((
                    Node {
                        width: Val::Px(MOBILITY_SIZE),
                        height: Val::Px(MOBILITY_SIZE),
                        ..default()
                    },
                    Interaction::default(),
                    TipFor::Mobility,
                ))
                .with_children(|slot| {
                    // `UI-Quickslot2` is the empty slot's 64×64 frame, the button 36 inside it.
                    slot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-MOBILITY_SIZE * 0.42),
                            top: Val::Px(-MOBILITY_SIZE * 0.42),
                            width: Val::Px(MOBILITY_SIZE * 1.84),
                            height: Val::Px(MOBILITY_SIZE * 1.84),
                            ..default()
                        },
                        ImageNode {
                            color: Color::NONE,
                            ..default()
                        },
                        UiArt::crop("Interface\\Buttons\\UI-Quickslot2", [0.0, 1.0, 0.0, 1.0]),
                    ));
                    slot.spawn((
                        MobilityIcon,
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        ImageNode {
                            color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                            ..default()
                        },
                    ));
                    let handle = sweeps_init.add(SweepUiMaterial { sweep: 0.0 });
                    slot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        MaterialNode(handle.clone()),
                        MobilitySweep(handle),
                    ));
                    slot.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            top: Val::Px(1.0),
                            right: Val::Px(3.0),
                            ..default()
                        },
                        text(f, "F", 12.0, Color::srgb(0.6, 0.6, 0.6)),
                    ));
                });
            });
            root.spawn(Node {
                width: Val::Percent(100.0),
                padding: UiRect::all(Val::Px(10.0)),
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::FlexStart,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(6.0),
                    width: Val::Px(300.0),
                    ..default()
                })
                .with_children(|col| {
                    // Under the unit frames (`frames.rs`).
                    col.spawn(Node {
                        height: Val::Px(84.0),
                        ..default()
                    });
                    col.spawn((
                        PassiveRow,
                        Node {
                            column_gap: Val::Px(4.0),
                            ..default()
                        },
                    ));
                    col.spawn((
                        ItemRow,
                        Node {
                            column_gap: Val::Px(3.0),
                            row_gap: Val::Px(3.0),
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        },
                    ));
                });
                // The clock, with the level and the kill count on one line under it.
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(0.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((TimerText, centered(f, "00:00", 34.0, Color::WHITE)));
                    col.spawn(Node {
                        column_gap: Val::Px(14.0),
                        ..default()
                    })
                    .with_children(|line| {
                        line.spawn((LevelText, text(f, "Level 1", 14.0, GOLD)));
                        line.spawn((KillText, text(f, "Kills 0", 14.0, Color::WHITE)));
                    });
                    col.spawn((
                        super::frames::NextBossText,
                        text(f, "", 14.0, Color::srgb(1.0, 0.45, 0.35)),
                    ));
                });
                // The minimap cluster's column.
                row.spawn(Node {
                    width: Val::Px(300.0),
                    ..default()
                });
            });
            super::frames::spawn_frames(root, f);
            root.spawn(Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(22.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            })
            .with_children(|n| {
                n.spawn((BannerText, centered(f, "", 30.0, GOLD)));
            });
            // The hero's health as a nameplate (`CGNamePlateFrame`): the bar under the border that
            // caps it, the level in the border's seat; [`hero_plate`] sizes and seats it.
            let absolute = || Node {
                position_type: PositionType::Absolute,
                ..default()
            };
            let unpainted = || ImageNode {
                color: Color::NONE,
                ..default()
            };
            let [r, g, b, _] = crate::vplates::PLATE_FRIENDLY;
            root.spawn((HpFrame, absolute(), Visibility::Hidden))
                .with_children(|plate| {
                    plate.spawn((HpBar, absolute())).with_children(|bar| {
                        bar.spawn((
                            HpFill,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            unpainted(),
                            UiArt::tinted(nameplate::BAR_FILL_TEXTURE, Color::srgb(r, g, b)),
                        ));
                        bar.spawn((
                            ShieldFill,
                            Node {
                                width: Val::Percent(0.0),
                                height: Val::Percent(100.0),
                                ..absolute()
                            },
                            unpainted(),
                            UiArt::tinted(
                                nameplate::BAR_FILL_TEXTURE,
                                Color::srgba(0.85, 0.9, 1.0, 0.75),
                            ),
                        ));
                    });
                    plate.spawn((
                        Node {
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            ..absolute()
                        },
                        unpainted(),
                        UiArt::crop(nameplate::BORDER_TEXTURE, [0.0, 1.0, 0.0, 1.0]),
                    ));
                    let [r, g, b, _] = crate::vplates::CON_YELLOW;
                    plate
                        .spawn((
                            HpLevelSeat,
                            Node {
                                width: Val::Px(0.0),
                                height: Val::Px(0.0),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                overflow: Overflow::visible(),
                                ..absolute()
                            },
                        ))
                        .with_children(|seat| {
                            seat.spawn((
                                HpLevel,
                                Text::new("1"),
                                TextFont {
                                    font: f.clone(),
                                    font_size: 11.0,
                                    ..default()
                                },
                                TextColor(Color::srgb(r, g, b)),
                                TextShadow {
                                    offset: Vec2::new(1.0, 1.0),
                                    color: Color::BLACK,
                                },
                                TextLayout::new(Justify::Center, LineBreak::NoWrap),
                            ));
                        });
                });
        });
    commands.insert_resource(map);

    // --- The run's pick screen; [`super::menu`] builds into it once the glue art is in. ---
    commands.spawn((
        MenuRoot,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        GlobalZIndex(410),
        bevy::ui::FocusPolicy::Pass,
    ));

    // --- Level up. ---
    commands
        .spawn((
            LevelUpRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            // A dark vignette closing in on the cards.
            BackgroundGradient::from(RadialGradient::new(
                UiPosition::CENTER,
                RadialGradientShape::FarthestCorner,
                vec![
                    ColorStop::percent(Color::srgba(0.04, 0.025, 0.0, 0.72), 0.0),
                    ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.9), 100.0),
                ],
            )),
            GlobalZIndex(420),
            Visibility::Hidden,
            // The build's icons stay hoverable under the cards.
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            // The slow golden burst behind everything.
            root.spawn((
                super::cards::Rays,
                Node {
                    position_type: PositionType::Absolute,
                    width: Val::Px(1400.0),
                    height: Val::Px(1400.0),
                    left: Val::Percent(50.0),
                    top: Val::Percent(50.0),
                    margin: UiRect::new(
                        Val::Px(-700.0),
                        Val::Px(0.0),
                        Val::Px(-700.0),
                        Val::Px(0.0),
                    ),
                    ..default()
                },
                BackgroundGradient(vec![
                    Gradient::Conic(ConicGradient::new(
                        UiPosition::CENTER,
                        (0..24)
                            .map(|i| {
                                let a = i as f32 / 24.0 * std::f32::consts::TAU;
                                let on = i % 2 == 0;
                                AngularColorStop::new(
                                    if on {
                                        Color::srgba(1.0, 0.8, 0.3, 0.07)
                                    } else {
                                        Color::srgba(1.0, 0.8, 0.3, 0.0)
                                    },
                                    a,
                                )
                            })
                            .collect(),
                    )),
                    Gradient::Radial(RadialGradient::new(
                        UiPosition::CENTER,
                        RadialGradientShape::ClosestSide,
                        vec![
                            ColorStop::percent(Color::srgba(1.0, 0.75, 0.25, 0.22), 0.0),
                            ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.0), 100.0),
                        ],
                    )),
                ]),
                UiTransform::default(),
                bevy::ui::FocusPolicy::Pass,
            ));
            // The level reached, on a gold tag over the title.
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(3.0)),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    margin: UiRect::bottom(Val::Px(-6.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.98, 0.82, 0.36)),
            ))
            .with_children(|b| {
                b.spawn((
                    LevelBadge,
                    text(f, "LEVEL 2", 15.0, Color::srgb(0.12, 0.08, 0.02)),
                ));
            });
            root.spawn((
                LevelUpTitle,
                centered(f, super::lang::tr("Level Up!"), 64.0, GOLD),
            ));
            root.spawn((
                LevelUpSub,
                centered(
                    f,
                    super::lang::tr("Choose a spell. Its rarity decides how strong it is."),
                    17.0,
                    Color::srgb(0.75, 0.72, 0.66),
                ),
            ));
            root.spawn((
                OfferRow,
                Node {
                    column_gap: Val::Px(22.0),
                    // Room for a hovered card's lift.
                    margin: UiRect::new(Val::ZERO, Val::ZERO, Val::Px(22.0), Val::Px(8.0)),
                    ..default()
                },
            ));
            super::cards::spawn_tools(root, f);
            root.spawn((
                RerollHint,
                centered(
                    f,
                    super::lang::tr("Click a card, or press 1 to 4"),
                    14.0,
                    DIM,
                ),
            ));
        });

    // --- Pause. ---
    commands
        .spawn((
            PauseRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
            GlobalZIndex(430),
            Visibility::Hidden,
            bevy::ui::FocusPolicy::Pass,
        ))
        .with_children(|root| {
            root.spawn(centered(f, "Paused", 46.0, GOLD));
            for (label, action) in [
                ("Resume", UiAction::Resume),
                ("Abandon run", UiAction::MainMenu),
                ("Quit game", UiAction::Quit),
            ] {
                root.spawn((
                    button(action, BUTTON),
                    Node {
                        width: Val::Px(240.0),
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(8.0)),
                        border: UiRect::all(Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                ))
                .with_children(|b| {
                    b.spawn(text(f, label, 20.0, Color::WHITE));
                });
            }
        });

    // --- Results. ---
    commands
        .spawn((
            OverRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
            BackgroundGradient::from(RadialGradient::new(
                UiPosition::CENTER,
                RadialGradientShape::FarthestCorner,
                vec![
                    ColorStop::percent(Color::srgba(0.07, 0.02, 0.0, 0.8), 0.0),
                    ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.94), 100.0),
                ],
            )),
            GlobalZIndex(440),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((OverTitle, centered(f, "", 64.0, GOLD)));
            root.spawn((
                OverLine,
                centered(f, "", 18.0, Color::srgb(0.85, 0.78, 0.6)),
            ));
            // The run in numbers, one tile each.
            root.spawn(Node {
                column_gap: Val::Px(12.0),
                margin: UiRect::vertical(Val::Px(14.0)),
                ..default()
            })
            .with_children(|row| {
                for (i, label) in ["Survived", "Level", "Kills", "Damage", "Bosses"]
                    .into_iter()
                    .enumerate()
                {
                    row.spawn((
                        Node {
                            width: Val::Px(128.0),
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(10.0), Val::Px(12.0)),
                            row_gap: Val::Px(4.0),
                            border: UiRect::all(Val::Px(1.0)),
                            border_radius: BorderRadius::all(Val::Px(8.0)),
                            ..default()
                        },
                        BackgroundGradient::from(LinearGradient::to_bottom(vec![
                            ColorStop::percent(Color::srgba(0.2, 0.14, 0.05, 0.92), 0.0),
                            ColorStop::percent(Color::srgba(0.04, 0.03, 0.02, 0.94), 100.0),
                        ])),
                        BorderColor::all(EDGE),
                    ))
                    .with_children(|tile| {
                        tile.spawn((OverStat(i), centered(f, "", 26.0, Color::WHITE)));
                        tile.spawn(centered(f, label, 12.0, DIM));
                    });
                }
            });
            root.spawn((OverStats, centered(f, "", 15.0, DIM)));
            root.spawn((
                edged_button(UiAction::MainMenu, BUTTON, GOLD),
                Node {
                    width: Val::Px(260.0),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(20.0), Val::Px(11.0)),
                    margin: UiRect::top(Val::Px(8.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BoxShadow::new(
                    Color::srgba(1.0, 0.7, 0.2, 0.35),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(0.0),
                    Val::Px(16.0),
                ),
            ))
            .with_children(|b| {
                b.spawn(text(f, "Play again", 22.0, GOLD));
            });
            root.spawn((
                button(UiAction::Quit, Color::NONE),
                Node {
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                    ..default()
                },
            ))
            .with_children(|b| {
                b.spawn(text(f, "Quit game", 15.0, DIM));
            });
        });
}

/// The menu's hero is the run's hero: respawned whenever the pick changes.
#[allow(clippy::too_many_arguments)]
pub(super) fn menu_preview(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    session: Res<super::Session>,
    selection: Res<Selection>,
    mut run: ResMut<Run>,
    heroes: Query<Entity, With<Hero>>,
    world: benilla_world::world_point::WorldPoint,
    catalog: Option<Res<crate::entities::CharCreate>>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    store: Res<super::profile::ProfileStore>,
    mut shown: Local<
        Option<(
            usize,
            (u32, u8, u8),
            usize,
            super::outfit::Appearance,
            bool,
            Vec<String>,
            Vec<(u8, u32)>,
        )>,
    >,
) {
    // A guest's run hero is the host's to send: no preview once its run has begun.
    if !matches!(phase.get(), Phase::Menu | Phase::Lobby) || session.mode == super::Mode::Client {
        return;
    }
    let want = selection
        .class
        .zip(selection.look.and_then(|i| selection.looks.get(i)).copied());
    let key = want.map(|(c, l)| {
        // The whole look: a race opened in the Rewards tab borrows a display another race has.
        (
            c,
            l,
            selection.stage,
            selection.appearance,
            catalog.is_some() && displays.is_some(),
            store
                .profile
                .gear()
                .iter()
                .map(|i| i.name.clone())
                .collect(),
            store.profile.cosmetics(),
        )
    });
    if key == *shown && (key.is_none() || !heroes.is_empty()) {
        return;
    }
    for e in &heroes {
        commands.entity(e).despawn();
    }
    *shown = key;
    let Some((class, (display, race, sex))) = want else {
        return;
    };
    let mut pos = hero::arena(selection.stage_def());
    if let Some(h) = world.terrain_height_under(pos) {
        pos.y = h;
    }
    let serial = run.guid();
    let class = &data::CLASSES[class];
    let body = super::outfit::body(
        class,
        race,
        sex,
        selection.appearance,
        display,
        &store.profile.gear(),
        &store.profile.cosmetics(),
        catalog.as_deref().map(|c| &c.0),
        displays.as_deref().map(|d| &d.catalog),
    );
    let e = hero::spawn_hero(&mut commands, class, &body, race, sex, serial, pos);
    commands.entity(e).insert(super::LocalHero);
}

/// A run begins with the hero picked at the menu: the view takes its fresh numbers, and the run's
/// heroes are seated on the first frame of play ([`super::sync::begin_run`]).
pub(super) fn start_run(run: &mut Run, selection: &mut Selection) -> bool {
    let (Some(class), Some(&(_, race, _))) = (
        selection.class_def(),
        selection.look.and_then(|i| selection.looks.get(i)),
    ) else {
        return false;
    };
    // The world goes to the battleground picked; until now it stood in the last one.
    selection.stage = selection.map;
    let next_guid = run.next_guid;
    let run_id = run.run_id.wrapping_add(1);
    *run = Run {
        me: Vitals::new(class, race),
        next_guid,
        run_id,
        ..Run::default()
    };
    true
}

pub(super) fn looks_for(
    class: &data::ClassDef,
    creatures: &crate::entities::Creatures,
) -> Vec<(u32, u8, u8)> {
    let mut out: Vec<(u32, u8, u8)> = Vec::new();
    for &d in class.looks {
        if let Some((race, sex)) = creatures.display_race_sex(d) {
            if !out.iter().any(|&(_, r, s)| r == race && s == sex) {
                out.push((d, race, sex));
            }
        }
    }
    out.sort_by_key(|&(_, r, s)| (r, s));
    out
}

#[allow(clippy::too_many_arguments)]
pub(super) fn buttons(
    mut presses: Query<
        (&Interaction, &UiAction, &BaseColor, &mut BackgroundColor),
        Changed<Interaction>,
    >,
    mut next: ResMut<NextState<Phase>>,
    mut exit: MessageWriter<AppExit>,
    phase: Res<State<Phase>>,
    session: Res<super::Session>,
) {
    for (interaction, action, base, mut bg) in &mut presses {
        match interaction {
            Interaction::Hovered => bg.0 = BUTTON_HOT,
            Interaction::None => bg.0 = base.0,
            Interaction::Pressed => {
                bg.0 = BUTTON_HOT;
                match (*action, *phase.get()) {
                    (UiAction::Resume, Phase::Paused) => next.set(Phase::Playing),
                    (UiAction::MainMenu, Phase::GameOver) => next.set(after_run(&session)),
                    // Mid-run, the pause menu's way out leaves a multiplayer lobby too.
                    (UiAction::MainMenu, Phase::Paused) => next.set(Phase::Menu),
                    (UiAction::Quit, Phase::Paused | Phase::GameOver) => {
                        exit.write(AppExit::Success);
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Where the results lead: the menu alone, the lobby with others.
fn after_run(session: &super::Session) -> Phase {
    if session.multiplayer() {
        Phase::Lobby
    } else {
        Phase::Menu
    }
}

pub(super) fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    focus: Res<super::kit::KitFocus>,
    phase: Res<State<Phase>>,
    session: Res<super::Session>,
    mut selection: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut next: ResMut<NextState<Phase>>,
    target: Option<ResMut<super::targeting::HeroTarget>>,
    sheet: Option<ResMut<super::sheet::SheetOpen>>,
) {
    // A name being typed keeps its keys.
    if focus.typing() {
        return;
    }
    match phase.get() {
        Phase::Menu => {
            if keys.just_pressed(KeyCode::Enter) {
                // Seated in a lobby, the menu only changes the hero: Enter goes back to it.
                if session.in_lobby {
                    next.set(Phase::Lobby);
                } else if start_run(&mut run, &mut selection) {
                    next.set(Phase::Playing);
                }
            }
        }
        Phase::Playing => {
            // C opens the character sheet over the paused run, as WoW's character pane.
            if keys.just_pressed(KeyCode::KeyC) {
                if let Some(mut sheet) = sheet {
                    sheet.0 = true;
                    next.set(Phase::Playing);
                }
                return;
            }
            if keys.just_pressed(KeyCode::Escape) {
                // As in WoW, Escape lets the target go first, and pauses on the next press.
                match target {
                    Some(mut t) if t.0.is_some() => t.0 = None,
                    _ => next.set(Phase::Paused),
                }
            }
            if keys.just_pressed(KeyCode::KeyP) {
                next.set(Phase::Paused);
            }
        }
        Phase::Paused => {
            if keys.just_pressed(KeyCode::KeyP) {
                next.set(Phase::Playing);
            }
        }
        // The lobby screen reads its own keys (`net::ui`).
        Phase::LevelUp | Phase::Lobby => {}
        Phase::GameOver => {
            if keys.just_pressed(KeyCode::Enter) {
                next.set(after_run(&session));
            }
        }
    }
}

/// The class's looks once the creature data is in, defaulting to the first. The auto-runner picks
/// a class with no look; the menu itself always sets both.
pub(super) fn refresh_menu(
    mut selection: ResMut<Selection>,
    creatures: Option<Res<crate::entities::Creatures>>,
) {
    if let (Some(class), Some(creatures)) = (selection.class_def(), creatures.as_deref()) {
        if selection.looks.is_empty() {
            let looks = looks_for(class, creatures);
            if !looks.is_empty() {
                selection.looks = looks;
                selection.look = Some(0);
                selection.version += 1;
            }
        }
    }
}

/// The badge at a card's top right.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Badge {
    /// A spell not yet known: filled in the rarity's colour.
    New,
    /// The rank the card brings a known spell to: outlined.
    Rank(u16),
    /// A shrine's blessing.
    Blessing,
    /// The healthstone.
    Heal,
}

/// What a level-up card shows.
pub(super) struct CardText {
    pub spell: u32,
    pub name: String,
    pub badge: Badge,
    /// Under the name: "New spell", "Rank 1 » 2".
    pub tag: String,
    /// What the spell does, for one not yet known; empty otherwise.
    pub desc: String,
    /// What the card grows, as `(value, what)`: `+25%` `damage`.
    pub perks: Vec<(String, &'static str)>,
    /// The card's foot: what it takes, or what it does.
    pub footer: String,
}

/// A card as it shows: a new spell with its first rank's numbers and what its rarity adds, a rank
/// with the stats it grows, a blessing with its stat.
pub(super) fn describe_card(
    choice: Choice,
    perks: &[data::Perk],
    run: &Run,
    spells: Option<&crate::ui_action::Spells>,
) -> CardText {
    use super::lang::{tr, trf};
    let perk_list = || perks.iter().map(|&p| perk_text(p)).collect::<Vec<_>>();
    match choice {
        Choice::NewAbility(def) => {
            let stats = run.stats();
            let c = cast_at(def, data::Perks::default(), &stats);
            CardText {
                spell: def.spell,
                name: spell_name(spells, def.spell, def.key),
                badge: Badge::New,
                tag: tr("New spell").to_string(),
                desc: format!("{}.", ability_body(def, &c)),
                perks: perk_list(),
                footer: trf(
                    "Takes a spell slot · {}/{}",
                    &[
                        &(run.abilities.len() + 1).to_string(),
                        &data::MAX_ABILITIES.to_string(),
                    ],
                ),
            }
        }
        Choice::Upgrade(k) => {
            let o = &run.abilities[k];
            let next = o.level.saturating_add(1);
            CardText {
                spell: o.def.spell,
                name: spell_name(spells, o.def.spell, o.def.key),
                badge: Badge::Rank(next),
                tag: trf("Rank {} » {}", &[&o.level.to_string(), &next.to_string()]),
                desc: String::new(),
                perks: perk_list(),
                footer: tr("Improves a spell you know").to_string(),
            }
        }
        // Passives are no longer dealt; a card naming one still reads.
        Choice::NewPassive(p) => CardText {
            spell: p.spell,
            name: p.name.into(),
            badge: Badge::New,
            tag: tr("New talent").to_string(),
            desc: passive_text(p.stat, p.per_level),
            perks: Vec::new(),
            footer: String::new(),
        },
        Choice::UpPassive(k) => {
            let (p, level, _) = run.passives[k];
            CardText {
                spell: p.spell,
                name: p.name.into(),
                badge: Badge::Rank(u16::from(level) + 1),
                tag: String::new(),
                desc: passive_text(p.stat, p.per_level),
                perks: Vec::new(),
                footer: String::new(),
            }
        }
        Choice::Boon(k) => {
            let b = run.shrine_offer.get(k).map_or(data::BOONS[0], |b| b.bonus);
            let v = match b.amount {
                data::Amount::Flat(v) | data::Amount::Percent(v) => v,
            };
            CardText {
                spell: data::shrine(0).spell,
                name: tr(stat_label(b.stat)).to_string(),
                badge: Badge::Blessing,
                tag: tr("Blessing").to_string(),
                desc: passive_text(b.stat, v),
                perks: Vec::new(),
                footer: tr("For the rest of the run").to_string(),
            }
        }
        Choice::ChestItem(k) => {
            let Some(item) = run.chest_offer.get(k).copied() else {
                return describe_card(Choice::Heal, perks, run, spells);
            };
            let held = run
                .run_items
                .iter()
                .find(|(i, _)| *i == item)
                .map_or(0, |(_, n)| *n);
            let b = item.bonus;
            let v = match b.amount {
                data::Amount::Flat(v) | data::Amount::Percent(v) => v,
            };
            let mut desc = passive_text(b.stat, v);
            if let Some(e) = item.effect {
                desc += " ";
                desc += &item_effect_text(e.at(held + 1));
            }
            CardText {
                spell: item.spell,
                name: tr(item.name).to_string(),
                badge: if held == 0 {
                    Badge::New
                } else {
                    Badge::Rank(u16::from(held) + 1)
                },
                tag: if held == 0 {
                    tr("New item").to_string()
                } else {
                    trf(
                        "Stack {} » {}",
                        &[&held.to_string(), &(held + 1).to_string()],
                    )
                },
                desc,
                perks: Vec::new(),
                footer: tr("For the rest of the run").to_string(),
            }
        }
        Choice::Heal => CardText {
            spell: 5720,
            name: tr("Healthstone").to_string(),
            badge: Badge::Heal,
            tag: tr("Restore").to_string(),
            desc: tr("Every spell is banished. Restores all health.").to_string(),
            perks: Vec::new(),
            footer: String::new(),
        },
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh_hud(
    mut commands: Commands,
    time: Res<Time>,
    phase: Res<State<Phase>>,
    mut run: ResMut<Run>,
    selection: Res<Selection>,
    font: Res<HudFont>,
    mut texts: ParamSet<(
        Query<&mut Text, With<LevelText>>,
        Query<&mut Text, With<TimerText>>,
        Query<&mut Text, With<KillText>>,
        Query<&mut Text, With<BannerText>>,
        Query<&mut Text, With<OverTitle>>,
        Query<&mut Text, With<OverStats>>,
        Query<&mut Text, With<XpText>>,
    )>,
    mut xp_fill: Query<&mut Node, With<XpFill>>,
    rows: Query<
        (Entity, Has<AbilityRow>, Has<ItemRow>),
        Or<(With<AbilityRow>, With<PassiveRow>, With<ItemRow>)>,
    >,
    mut sweeps: ResMut<Assets<SweepUiMaterial>>,
    mut last_version: Local<Option<u32>>,
) {
    let need = data::xp_to_next(run.level);
    if let Ok(mut n) = xp_fill.single_mut() {
        n.width = Val::Percent((run.xp / need * 100.0).clamp(0.0, 100.0));
    }
    if let Ok(mut t) = texts.p6().single_mut() {
        let xp = format!("XP {} / {}", run.xp as u32, need as u32);
        if **t != xp {
            **t = xp;
        }
    }
    if let Ok(mut t) = texts.p0().single_mut() {
        **t = format!("Level {}", run.level);
    }
    // The stage's clock, or what is left of the final swarm.
    let clock = match run.swarm {
        Some(left) => {
            let s = left.ceil().max(0.0) as u32;
            super::lang::trf("Swarm {}", &[&format!("{}:{:02}", s / 60, s % 60)])
        }
        None => {
            let secs = run.elapsed as u32;
            format!("{:02}:{:02}", secs / 60, secs % 60)
        }
    };
    if let Ok(mut t) = texts.p1().single_mut() {
        if **t != clock {
            **t = clock;
        }
    }
    let kills = run.kills;
    if let Ok(mut t) = texts.p2().single_mut() {
        let mut want = super::lang::trf(
            "Stage {}/{}  ·  ",
            &[&(run.leg + 1).to_string(), &data::STAGES.to_string()],
        );
        want += &super::lang::trf(
            "Kills {}  ·  Gold {}  ·  Chest {}",
            &[
                &kills.to_string(),
                &(run.gold.floor() as u32).to_string(),
                &run.chest_cost.to_string(),
            ],
        );
        if run.keys > 0 {
            want.push_str(&super::lang::trf("  ·  Keys {}", &[&run.keys.to_string()]));
        }
        if **t != want {
            **t = want;
        }
    }
    let banner = match run.banner.as_mut() {
        Some((msg, t)) => {
            *t -= time.delta_secs();
            if *t > 0.0 {
                super::lang::tr(msg).to_string()
            } else {
                String::new()
            }
        }
        None => String::new(),
    };
    if banner.is_empty() {
        run.banner = None;
    }
    // The picker owns the screen: an announcement waits until it closes.
    let banner = if *phase.get() == Phase::LevelUp {
        String::new()
    } else {
        banner
    };
    if let Ok(mut t) = texts.p3().single_mut() {
        if **t != banner {
            **t = banner;
        }
    }
    let title = super::lang::tr(if run.won {
        "Victory!"
    } else {
        "You have fallen"
    })
    .to_string();
    if let Ok(mut t) = texts.p4().single_mut() {
        **t = title;
    }
    let summary = format!(
        "{} {} in {}   —   press Enter to play again",
        crate::char_select::race_name(run.race),
        run.class.name,
        selection.map_def().name,
    );
    if let Ok(mut t) = texts.p5().single_mut() {
        if **t != summary {
            **t = summary;
        }
    }

    // The ability and passive icons, rebuilt when the build changes.
    if *last_version == Some(run.version) {
        return;
    }
    *last_version = Some(run.version);
    let f = &font.0;
    for (row, is_abilities, is_items) in &rows {
        commands.entity(row).despawn_related::<Children>();
        commands.entity(row).with_children(|row| {
            let entries: Vec<(u32, u16, f32, TipFor, Color)> = if is_abilities {
                run.abilities
                    .iter()
                    .enumerate()
                    .map(|(i, o)| {
                        (
                            o.def.spell,
                            o.level,
                            super::frames::SLOT,
                            TipFor::Ability(i),
                            EDGE,
                        )
                    })
                    .collect()
            } else if is_items {
                // Each item edged in its rarity's colour, its stacks in the corner.
                run.run_items
                    .iter()
                    .enumerate()
                    .map(|(i, (item, n))| {
                        let edge = super::cards::rarity_color(item.rarity);
                        (item.spell, u16::from(*n), 26.0, TipFor::RunItem(i), edge)
                    })
                    .collect()
            } else {
                run.passives
                    .iter()
                    .enumerate()
                    .map(|(i, (p, l, _))| (p.spell, u16::from(*l), 30.0, TipFor::Passive(i), EDGE))
                    .collect()
            };
            for (spell, level, size, tip, edge) in entries {
                let ability = match &tip {
                    TipFor::Ability(i) => Some(*i),
                    _ => None,
                };
                row.spawn((
                    Node {
                        width: Val::Px(size),
                        height: Val::Px(size),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(edge),
                    Interaction::default(),
                    tip,
                ))
                .with_children(|slot| {
                    slot.spawn(icon(spell, size - 2.0));
                    // The cooldown sweep sits over the icon, under the level text.
                    if let Some(i) = ability {
                        let handle = sweeps.add(SweepUiMaterial { sweep: 0.0 });
                        slot.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Percent(100.0),
                                height: Val::Percent(100.0),
                                ..default()
                            },
                            MaterialNode(handle.clone()),
                            AbilitySweep { handle, index: i },
                        ));
                    }
                    slot.spawn(Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(2.0),
                        bottom: Val::Px(0.0),
                        ..default()
                    })
                    .with_children(|n| {
                        n.spawn(text(f, level.to_string(), 12.0, Color::WHITE));
                    });
                });
            }
        });
    }
}

/// Fill icon slots from `SpellIcon.dbc` once the client data and the asset reader are up.
pub(super) fn fill_icons(
    mut commands: Commands,
    mut slots: Query<(Entity, &mut ImageNode, &SpellIcon), Without<IconDone>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut cache: Local<HashMap<u32, Handle<Image>>>,
) {
    let (Some(spells), Some(assets)) = (spells.as_deref(), assets.as_deref_mut()) else {
        return;
    };
    for (e, mut img, icon) in &mut slots {
        let handle = match cache.get(&icon.0) {
            Some(h) => Some(h.clone()),
            None => {
                let path = spells
                    .catalog
                    .get(icon.0)
                    .and_then(|d| d.icon.clone())
                    .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".into());
                let h = assets.sprite_texture(&format!("{path}.blp"), &mut images);
                if let Some(h) = &h {
                    cache.insert(icon.0, h.clone());
                }
                h
            }
        };
        if let Some(h) = handle {
            img.image = h;
            img.color = Color::WHITE;
        }
        commands.entity(e).insert(IconDone);
    }
}

/// The mobility button: shown once learned, its icon the class's spell, its sweep the cooldown.
#[allow(clippy::type_complexity)]
pub(super) fn refresh_mobility(
    run: Res<Run>,
    mut commands: Commands,
    mut slot: Query<&mut Node, With<MobilitySlot>>,
    icon: Query<(Entity, Option<&SpellIcon>), With<MobilityIcon>>,
    sweep: Query<&MobilitySweep>,
    mut mats: ResMut<Assets<SweepUiMaterial>>,
) {
    let def = run.mobility();
    if let Ok(mut node) = slot.single_mut() {
        let display = if def.is_some() {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != display {
            node.display = display;
        }
    }
    let Some(def) = def else { return };
    // A new spell re-arms the icon fill ([`fill_icons`]).
    if let Ok((e, have)) = icon.single() {
        if have.is_none_or(|s| s.0 != def.spell) {
            commands
                .entity(e)
                .remove::<IconDone>()
                .insert(SpellIcon(def.spell));
        }
    }
    if let Ok(s) = sweep.single() {
        let f = (run.mobile.cd / def.cooldown.max(0.01)).clamp(0.0, 1.0);
        if let Some(m) = mats.get_mut(&s.0) {
            if m.sweep != f {
                m.sweep = f;
            }
        }
    }
}

/// Drive each ability icon's cooldown sweep from its remaining cooldown.
pub(super) fn update_cooldown_sweeps(
    run: Res<Run>,
    mut mats: ResMut<Assets<SweepUiMaterial>>,
    sweeps: Query<&AbilitySweep>,
) {
    let stats = run.stats();
    for s in &sweeps {
        let Some(o) = run.abilities.get(s.index) else {
            continue;
        };
        let total = o.cooldown(&stats).max(0.01);
        let f = (o.cd / total).clamp(0.0, 1.0);
        if let Some(m) = mats.get_mut(&s.handle) {
            m.sweep = f;
        }
    }
}

/// Draw each [`UiArt`] region once its file can be read.
pub(super) fn fill_art(
    mut commands: Commands,
    mut regions: Query<(Entity, &mut ImageNode, &UiArt), Without<ArtDone>>,
    assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(mut assets) = assets else { return };
    for (e, mut img, art) in &mut regions {
        commands.entity(e).insert(ArtDone);
        let Some(h) = assets.sprite_texture(art.path, &mut images) else {
            warn!("survivors: no UI art {}", art.path);
            continue;
        };
        let [l, r, t, b] = art.coords;
        if let Some(size) = images.get(&h).map(Image::size_f32) {
            img.rect = Some(Rect::new(l * size.x, t * size.y, r * size.x, b * size.y));
        }
        img.image = h;
        img.color = art.tint;
    }
}

/// The reference lays its UI out on a screen 768 units tall at any resolution (`useUiScale` off).
pub(super) fn apply_ui_scale(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
) {
    let Ok(window) = windows.single() else { return };
    let want = window.height() / 768.0;
    if want > 0.0 && (scale.0 - want).abs() > 1e-4 {
        scale.0 = want;
    }
}

/// Between the hero's feet and the top of its plate's bar, in gx units.
const PLATE_FEET_GAP: f32 = 0.004;
/// A couple of screen pixels lower than the gap alone sets it.
const PLATE_NUDGE: f32 = 2.0;

/// The hero's nameplate under its feet, sized from the screen diagonal as [`crate::vplates`]
/// sizes a plate, outside the UI scale. The border's top half seats a name the hero has none of,
/// so the plate rises until its bar sits just under the feet.
#[allow(clippy::type_complexity)]
pub(super) fn hero_plate(
    run: Res<Run>,
    ui_scale: Res<UiScale>,
    hero: Query<&Transform, With<super::LocalHero>>,
    cams: Query<(&Camera, &Transform), With<WorldCamera>>,
    mut nodes: ParamSet<(
        Query<(&mut Node, &mut Visibility), With<HpFrame>>,
        Query<&mut Node, With<HpBar>>,
        Query<&mut Node, With<HpFill>>,
        Query<&mut Node, With<ShieldFill>>,
        Query<&mut Node, With<HpLevelSeat>>,
    )>,
    mut level: Query<(&mut Text, &mut TextFont), With<HpLevel>>,
) {
    use crate::vplates::{
        gx_px, plate_basis, text_px, BAR_H, BAR_OFF_X, BAR_OFF_Y, BAR_W, LEVEL_H, LEVEL_OFF_X,
        LEVEL_OFF_Y, PLATE_H, PLATE_W,
    };
    let seat = hero.single().ok().and_then(|h| {
        let (cam, tf) = cams.single().ok()?;
        let viewport = cam.logical_viewport_size()?;
        let feet = cam
            .world_to_viewport(&GlobalTransform::from(*tf), h.translation)
            .ok()?;
        Some((feet, viewport))
    });
    let mut frames = nodes.p0();
    let Ok((mut frame, mut vis)) = frames.single_mut() else {
        return;
    };
    let Some((feet, viewport)) = seat else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    let s = ui_scale.0;
    let basis = plate_basis(viewport);
    let gx = |v: f32| gx_px(v, basis) / s;
    let (pw, ph) = (gx(PLATE_W), gx(PLATE_H));
    let bar_top = ph - gx(BAR_OFF_Y) - gx(BAR_H);
    frame.width = Val::Px(pw);
    frame.height = Val::Px(ph);
    frame.left = Val::Px(feet.x / s - pw / 2.0);
    frame.top = Val::Px(feet.y / s + gx(PLATE_FEET_GAP) - bar_top + PLATE_NUDGE / s);
    if let Ok(mut bar) = nodes.p1().single_mut() {
        bar.left = Val::Px(gx(BAR_OFF_X));
        bar.top = Val::Px(bar_top);
        bar.width = Val::Px(gx(BAR_W));
        bar.height = Val::Px(gx(BAR_H));
    }
    let stats = run.stats();
    let hp = (run.hp / stats.max_hp).clamp(0.0, 1.0);
    let shield = if run.shield_t > 0.0 {
        (run.shield / stats.max_hp).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if let Ok(mut n) = nodes.p2().single_mut() {
        n.width = Val::Percent(hp * 100.0);
    }
    if let Ok(mut n) = nodes.p3().single_mut() {
        n.width = Val::Percent(shield * 100.0);
    }
    if let Ok(mut seat) = nodes.p4().single_mut() {
        seat.left = Val::Px(pw - gx(LEVEL_OFF_X));
        seat.top = Val::Px(ph - gx(LEVEL_OFF_Y));
    }
    if let Ok((mut text, mut font)) = level.single_mut() {
        let n = run.level.to_string();
        if text.0 != n {
            text.0 = n;
        }
        let size = text_px(LEVEL_H, basis) / s;
        if font.font_size != size {
            font.font_size = size;
        }
    }
}

/// `MainMenuExpBar`'s `OnEnter` and `OnLeave`: the numbers show while the cursor is on the bar.
pub(super) fn xp_hover(
    bars: Query<&Interaction, (With<XpBar>, Changed<Interaction>)>,
    mut texts: Query<&mut Visibility, With<XpText>>,
) {
    for interaction in &bars {
        let want = if *interaction == Interaction::None {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        for mut v in &mut texts {
            *v = want;
        }
    }
}

// ── The build icons' `GameTooltip` ──────────────────────────────────────────────────────────────

/// `TOOLTIP_DEFAULT_BACKGROUND_COLOR`, the `GameTooltip` backdrop tint (`GameTooltip.lua`).
const TIP_FILL: Color = Color::srgb(0.09, 0.09, 0.19);
/// The grey a spell tooltip prints its rank in, right of the name.
const TIP_GREY: Color = Color::srgb(0.5, 0.5, 0.5);
/// A tooltip's bonuses, WoW's green.
const TIP_GREEN: Color = Color::srgb(0.12, 1.0, 0.12);

/// One tooltip line: the left text, an optional right-aligned one, the colour and font size
/// (`GameTooltipHeaderText` 14, `GameTooltipText` 12).
#[derive(Clone)]
pub(super) struct TipLine {
    left: String,
    right: Option<(String, Color)>,
    color: Color,
    size: f32,
}

impl TipLine {
    pub(super) fn new(left: impl Into<String>, color: Color) -> Self {
        Self {
            left: left.into(),
            right: None,
            color,
            size: 12.0,
        }
    }

    pub(super) fn right(mut self, text: impl Into<String>, color: Color) -> Self {
        self.right = Some((text.into(), color));
        self
    }

    pub(super) fn header(mut self) -> Self {
        self.size = 14.0;
        self
    }
}

/// A number as the tooltips print it: whole when near enough, else to one decimal.

/// The results screen's tiles and its line under the title.
#[allow(clippy::type_complexity)]
pub(super) fn refresh_results(
    run: Res<Run>,
    selection: Res<Selection>,
    mut over_texts: ParamSet<(
        Query<(&OverStat, &mut Text)>,
        Query<&mut Text, With<OverLine>>,
    )>,
) {
    let secs = run.elapsed as u32;
    let mut stats_line = if run.won {
        selection.map_def().victory
    } else {
        super::lang::tr("The horde was too much this time.")
    }
    .to_string();
    let reward = data::DIFFICULTIES[run.difficulty.min(data::DIFFICULTIES.len() - 1)].reward;
    let gold = data::run_gold(run.kills, run.bosses, run.won, run.stats().gold * reward)
        + (run.gold.max(0.0) * data::UNSPENT_GOLD_KEPT) as u32;
    if gold > 0 {
        stats_line.push('\n');
        stats_line.push_str(&super::lang::trf("Gold earned: {}", &[&gold.to_string()]));
    }
    if !run.run_items.is_empty() {
        let items: Vec<String> = run
            .run_items
            .iter()
            .map(|(i, n)| format!("{} ×{n}", i.name))
            .collect();
        stats_line.push('\n');
        stats_line.push_str(&super::lang::trf("Run items: {}", &[&items.join(", ")]));
    }
    // What was found comes home either way.
    if !run.loot.is_empty() {
        let names: Vec<&str> = run.loot.iter().map(|i| i.name.as_str()).collect();
        stats_line.push('\n');
        stats_line.push_str(&super::lang::trf("Found: {}", &[&names.join(", ")]));
    }
    let values = [
        format!("{:02}:{:02}", secs / 60, secs % 60),
        run.level.to_string(),
        run.kills.to_string(),
        compact(run.damage_done),
        run.bosses.to_string(),
    ];
    for (stat, mut t) in &mut over_texts.p0() {
        if let Some(v) = values.get(stat.0) {
            if **t != *v {
                **t = v.clone();
            }
        }
    }
    if let Ok(mut t) = over_texts.p1().single_mut() {
        if **t != stats_line {
            **t = stats_line;
        }
    }
}

/// 12.3k, 4.5M.
fn compact(v: f64) -> String {
    if v >= 1e6 {
        format!("{:.1}M", v / 1e6)
    } else if v >= 1e4 {
        format!("{:.1}k", v / 1e3)
    } else {
        format!("{}", v.round() as u64)
    }
}

pub(super) fn num(v: f32) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

pub(super) fn pct(v: f32) -> String {
    format!("{}%", num(v * 100.0))
}

/// The numbers an ability casts with at a level, as [`super::abilities::cast_abilities`] takes
/// them from [`Owned`].
struct Cast {
    dmg: f32,
    dot: f32,
    count: u32,
    radius: f32,
    duration: f32,
    cooldown: f32,
}

/// What `def` casts with `perks` from its cards.
fn cast_at(def: &'static AbilityDef, perks: data::Perks, stats: &Stats) -> Cast {
    let o = Owned {
        def,
        level: 1,
        cd: 0.0,
        perks,
    };
    let mult = o.damage_mult() * stats.might;
    Cast {
        dmg: def.damage * mult,
        dot: def.dot * mult,
        count: o.count(),
        radius: o.radius(stats),
        duration: o.duration(),
        cooldown: o.cooldown(stats),
    }
}

fn effect_text(effect: Effect) -> Option<String> {
    Some(match effect {
        Effect::None => return None,
        Effect::Slow(m, t) => format!("slowing them by {} for {} sec", pct(1.0 - m), num(t)),
        Effect::Root(t) => format!("freezing them in place for {} sec", num(t)),
        Effect::Stun(t) => format!("stunning them for {} sec", num(t)),
        Effect::Fear(t) => format!("sending them fleeing for {} sec", num(t)),
        Effect::Weaken(m, t) => format!(
            "cutting their damage by {} for {} sec",
            pct(1.0 - m),
            num(t)
        ),
        Effect::Drain(f) => format!("healing you for {} of the damage", pct(f)),
        Effect::Execute(f) => format!("killing them outright below {} health", pct(f)),
    })
}

/// What an ability does at these numbers, in the voice of a spell description.
fn ability_body(def: &AbilityDef, c: &Cast) -> String {
    let d = num(c.dmg);
    let burst = def.duration <= 0.0;
    let mut s = match def.kind {
        Kind::Bolt | Kind::Strike | Kind::Melee => {
            let toughest = matches!(def.key, "pyroblast" | "aimed_shot" | "exorcism" | "starfire");
            let melee = def.kind == Kind::Melee;
            let who = match (toughest, melee, c.count) {
                (true, _, 1) => "the toughest enemy in range".to_string(),
                (true, _, n) => format!("the {n} toughest enemies in range"),
                (false, true, 1) => "the closest enemy".into(),
                (false, true, n) => format!("the {n} closest enemies"),
                (false, false, 1) => "the nearest enemy".into(),
                (false, false, n) => format!("the {n} nearest enemies"),
            };
            let mut s = format!("{} {who} for {d} damage", if melee { "Strikes" } else { "Hits" });
            if c.radius > 0.0 && !melee {
                s += &format!(", and everything within {} yd of the hit", num(c.radius));
            }
            s
        }
        Kind::Nova => format!("Deals {d} damage to every enemy within {} yd of you", num(c.radius)),
        Kind::Cone => format!("Deals {d} damage to every enemy in a {} yd cone toward the closest", num(c.radius)),
        Kind::Ground | Kind::GroundSelf => {
            let at = if def.kind == Kind::GroundSelf { "around you" } else { "on the thickest pack" };
            if burst {
                format!("Strikes a {} yd area {at} for {d} damage", num(c.radius))
            } else {
                format!(
                    "Covers a {} yd area {at} for {} sec, dealing {d} damage every second",
                    num(c.radius),
                    num(c.duration)
                )
            }
        }
        Kind::Chain => format!(
            "Strikes the nearest enemy for {d} damage, then jumps to {} more within {} yd of each other, 15% weaker with every jump",
            c.count,
            num(c.radius)
        ),
        Kind::Dot => format!(
            "Afflicts {} enemies in range with {} damage every second for {} sec",
            c.count,
            num(c.dot),
            num(c.duration)
        ),
        Kind::Aura => format!(
            "Every {} sec, deals {d} damage to every enemy within {} yd of you",
            num(c.cooldown),
            num(c.radius)
        ),
        Kind::Orbit => format!(
            "Every {} sec, zaps {} within {} yd of you for {d} damage",
            num(c.cooldown),
            if c.count == 1 { "an enemy".to_string() } else { format!("{} enemies", c.count) },
            num(c.radius)
        ),
        Kind::Shield => format!("Absorbs the next {d} damage you take within {} sec", num(c.duration)),
        Kind::Immune => format!(
            "When an enemy closes in, makes you immune to all damage for {} sec",
            num(c.duration)
        ),
        Kind::Thorns => format!(
            "When enemies close in, deals {d} damage to every attacker for {} sec",
            num(c.duration)
        ),
        Kind::Heal => format!("Below 85% health, heals you for {d} over {} sec", num(c.duration)),
        Kind::Totem if def.key == "magma_totem" => format!(
            "Drops a totem for {} sec that burns every enemy within {} yd for {d} damage every 2 sec",
            num(c.duration),
            num(c.radius)
        ),
        Kind::Totem => format!(
            "Drops a totem for {} sec that hits the nearest enemy within {} yd for {d} damage every second",
            num(c.duration),
            num(def.range)
        ),
    };
    // Areas that tick carry no over-time part; a burst leaves a 5 second one.
    let ground = matches!(def.kind, Kind::Ground | Kind::GroundSelf);
    if c.dot > 0.0 && def.kind != Kind::Dot && (!ground || burst) {
        let secs = if ground { 5.0 } else { c.duration };
        s += &format!(
            ", then {} damage every second for {} sec",
            num(c.dot),
            num(secs)
        );
    }
    if let Some(e) = effect_text(def.effect) {
        s += ", ";
        s += &e;
    }
    s.push('.');
    s
}

/// What the next rank changes, at the current stats.
pub(super) fn ability_tip(
    o: &Owned,
    stats: &Stats,
    spells: Option<&crate::ui_action::Spells>,
) -> Vec<TipLine> {
    use super::lang::{tr, trf};
    let def = o.def;
    let now = cast_at(def, o.perks, stats);
    let mut lines = vec![
        TipLine::new(spell_name(spells, def.spell, def.key), Color::WHITE)
            .header()
            .right(format!("Rank {}", o.level), TIP_GREY),
    ];
    // As WoW's spell tooltip: its range, then how it casts and its cooldown, then what it does.
    let range = match def.kind {
        Kind::Melee => Some(tr("Melee Range").to_string()),
        Kind::Bolt | Kind::Strike | Kind::Chain | Kind::Dot | Kind::Ground | Kind::Totem => {
            Some(trf("{} yd range", &[&num(def.range)]))
        }
        _ => None,
    };
    if let Some(range) = range {
        lines.push(TipLine::new(range, Color::WHITE));
    }
    lines.push(
        TipLine::new(tr("Automatic"), Color::WHITE)
            .right(trf("{} sec cooldown", &[&num(now.cooldown)]), Color::WHITE),
    );
    lines.push(TipLine::new(ability_body(def, &now), GOLD));
    // What its cards have given it so far, in green as WoW's bonuses.
    let grown: Vec<String> = perk_lines(&o.perks)
        .into_iter()
        .map(|(v, l)| format!("{v} {l}"))
        .collect();
    if !grown.is_empty() {
        lines.push(TipLine::new(
            trf("Your bonuses: {}", &[&grown.join(", ")]),
            TIP_GREEN,
        ));
    }
    if !matches!(
        def.kind,
        Kind::Shield | Kind::Immune | Kind::Heal | Kind::Dot
    ) {
        lines.push(TipLine::new(
            trf(
                "{} chance to crit for {} damage",
                &[&pct(stats.crit), &pct(data::CRIT_MULT)],
            ),
            Color::WHITE,
        ));
    }
    lines
}

/// A ware of the merchant's: the item as it would be once bought, and its price.
fn ware_tip(w: &super::market::Ware, run: &Run, stats: &Stats) -> Vec<TipLine> {
    use super::lang::{tr, trf};
    let held = run
        .run_items
        .iter()
        .find(|(i, _)| *i == w.item)
        .map_or(0, |(_, n)| *n);
    let after = (held + 1).min(data::MAX_RUN_ITEM_STACKS);
    let mut lines = item_tip(w.item, after, stats);
    let (line, color) = if w.sold {
        (tr("Sold").to_string(), DIM)
    } else if held >= data::MAX_RUN_ITEM_STACKS {
        (tr("Most stacks: it cannot be bought").to_string(), DIM)
    } else if held == 0 && run.run_items.len() >= data::MAX_RUN_ITEM_KINDS {
        (tr("Five kinds held: it cannot be bought").to_string(), DIM)
    } else if run.gold + 1e-3 >= w.price as f32 {
        (trf("Buy for {} gold", &[&w.price.to_string()]), TIP_GREEN)
    } else {
        (
            trf("Costs {} gold", &[&w.price.to_string()]),
            Color::srgb(0.85, 0.3, 0.25),
        )
    };
    lines.push(TipLine::new(line, color));
    lines
}

/// One perk as a card shows it: its value (`+25%`, `-10%`, `+1`) and what it grows.
pub(super) fn perk_text(p: data::Perk) -> (String, &'static str) {
    use super::lang::tr;
    let share = |v: f32| format!("{}%", (v * 100.0).round() as i32);
    match p {
        data::Perk::Damage(v) => (format!("+{}", share(v)), tr("damage")),
        data::Perk::Cooldown(v) => (format!("-{}", share(v)), tr("cooldown")),
        data::Perk::Targets(n) => (format!("+{n}"), tr("targets")),
        data::Perk::Area(v) => (format!("+{}", share(v)), tr("area")),
        data::Perk::Duration(v) => (format!("+{}", share(v)), tr("duration")),
    }
}

/// What a spell's cards have given it, summed, as lines: only what moved.
fn perk_lines(p: &data::Perks) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    if p.damage > 0.0 {
        out.push(perk_text(data::Perk::Damage(p.damage)));
    }
    if p.cooldown < 1.0 {
        out.push(perk_text(data::Perk::Cooldown(1.0 - p.cooldown)));
    }
    if p.targets > 0 {
        out.push(perk_text(data::Perk::Targets(p.targets)));
    }
    if p.area > 0.0 {
        out.push(perk_text(data::Perk::Area(p.area)));
    }
    if p.duration > 0.0 {
        out.push(perk_text(data::Perk::Duration(p.duration)));
    }
    out
}

/// What a passive's bonus `v` does, in the voice of a buff.
/// A stat's short name, for the card's before » after line.
fn stat_label(stat: Stat) -> &'static str {
    match stat {
        Stat::Might => "All damage",
        Stat::MaxHp => "Maximum health",
        Stat::Speed => "Movement speed",
        Stat::Regen => "Health per second",
        Stat::Haste => "Recharge speed",
        Stat::Area => "Area size",
        Stat::Magnet => "Pickup range",
        Stat::Growth => "Experience gained",
        Stat::Armor => "Damage blocked per hit",
        Stat::Crit => "Critical strike chance",
        Stat::DamageReduction => "Damage reduction",
        Stat::Gold => "Gold found",
    }
}

fn passive_text(stat: Stat, v: f32) -> String {
    match stat {
        Stat::Might => format!("Increases all damage you deal by {}.", pct(v)),
        Stat::MaxHp => format!("Increases your maximum health by {}.", pct(v)),
        Stat::Speed => format!("Increases your movement speed by {}.", pct(v)),
        Stat::Regen => format!("Restores {} health every second.", num(v)),
        Stat::Haste => format!("Your abilities recharge {} faster.", pct(v)),
        Stat::Area => format!("Increases the size of your areas of effect by {}.", pct(v)),
        Stat::Magnet => format!("Increases your pickup range by {}.", pct(v)),
        Stat::Growth => format!("Increases the experience you gain by {}.", pct(v)),
        Stat::Armor => format!("Reduces the damage of every hit you take by {}.", num(v)),
        Stat::Crit => format!(
            "Increases your chance to critically strike for double damage by {}.",
            pct(v)
        ),
        Stat::DamageReduction => format!("Reduces all damage you take by {}.", pct(v)),
        Stat::Gold => format!("Increases the gold you find by {}.", pct(v)),
    }
}

/// The hero's total for the stat a passive raises, every source counted.
fn stat_total(stat: Stat, s: &Stats) -> String {
    match stat {
        Stat::Might => format!("Damage dealt: {}", pct(s.might)),
        Stat::MaxHp => format!("Maximum health: {}", num(s.max_hp)),
        Stat::Speed => format!("Movement speed: {} yd per second", num(s.speed)),
        Stat::Regen => format!("Health restored: {} per second", num(s.regen)),
        Stat::Haste => format!("Cooldowns: {} shorter", pct(s.haste.min(0.65))),
        Stat::Area => format!("Area of effect: {}", pct(s.area)),
        Stat::Magnet => format!("Pickup range: {} yd", num(s.magnet)),
        Stat::Growth => format!("Experience gained: {}", pct(s.growth)),
        Stat::Armor => format!("Damage blocked per hit: {}", num(s.armor)),
        Stat::Crit => format!("Critical strike chance: {}", pct(s.crit)),
        Stat::DamageReduction => format!("Damage reduction: {}", pct(s.damage_reduction)),
        Stat::Gold => format!("Gold found: {}", pct(s.gold)),
    }
}

fn passive_tip(p: &PassiveDef, level: u8, power: f32, stats: &Stats) -> Vec<TipLine> {
    let mut lines = vec![
        TipLine::new(p.name, Color::WHITE)
            .header()
            .right(format!("Rank {level}"), TIP_GREY),
        TipLine::new(passive_text(p.stat, p.per_level * power), GOLD),
        TipLine::new(stat_total(p.stat, stats), Color::WHITE),
    ];
    if level < data::MAX_PASSIVE_LEVEL {
        lines.push(TipLine::new("Next rank:", Color::WHITE));
        lines.push(TipLine::new(
            passive_text(p.stat, p.per_level * (power + 1.0)),
            GOLD,
        ));
    } else {
        lines.push(TipLine::new("Highest rank", DIM));
    }
    lines
}

/// A run item's tooltip: its name in its rarity's colour and its stacks, what its stacks give
/// and do now, and how many more it can take.
pub(super) fn item_tip(item: &data::RunItemDef, stacks: u8, stats: &Stats) -> Vec<TipLine> {
    use super::lang::tr;
    let color = super::cards::rarity_color(item.rarity);
    let b = super::stats::scaled(item.bonus, f32::from(stacks));
    let v = match b.amount {
        data::Amount::Flat(v) | data::Amount::Percent(v) => v,
    };
    let mut lines = vec![
        TipLine::new(item.name, color)
            .header()
            .right(format!("×{stacks}"), Color::WHITE),
        TipLine::new(super::cards::rarity_label(item.rarity), color),
        TipLine::new(passive_text(b.stat, v), GOLD),
    ];
    if let Some(e) = item.effect {
        lines.push(TipLine::new(item_effect_text(e.at(stacks)), GOLD));
    }
    lines.push(TipLine::new(stat_total(b.stat, stats), Color::WHITE));
    if stacks >= data::MAX_RUN_ITEM_STACKS {
        lines.push(TipLine::new(tr("Most stacks"), DIM));
    }
    lines
}

/// What an item's effect does, at the numbers it has now.
pub(super) fn item_effect_text(e: data::ItemEffect) -> String {
    use super::lang::trf;
    match e {
        data::ItemEffect::Lifesteal(share) => {
            trf("Heals you for {} of the damage you deal.", &[&pct(share)])
        }
        data::ItemEffect::Chain {
            chance,
            damage,
            jumps,
            ..
        } => trf(
            "{} chance on a hit to arc to {} more foes for {} damage.",
            &[&pct(chance), &jumps.to_string(), &num(damage)],
        ),
        data::ItemEffect::Thorns(d) => {
            trf("Returns {} damage to whatever strikes you.", &[&num(d)])
        }
        data::ItemEffect::Burst {
            chance,
            damage,
            radius,
            ..
        } => trf(
            "{} chance on a kill to burst for {} damage within {} yd.",
            &[&pct(chance), &num(damage), &num(radius)],
        ),
        data::ItemEffect::KillHeal(hp) => trf("Each kill heals you for {}.", &[&num(hp)]),
        data::ItemEffect::Ignite {
            chance, dps, secs, ..
        } => trf(
            "{} chance on a hit to burn the foe for {} damage a second over {} sec.",
            &[&pct(chance), &num(dps), &num(secs)],
        ),
        data::ItemEffect::Chill { chance, slow, secs } => trf(
            "{} chance on a hit to slow the foe by {} for {} sec.",
            &[&pct(chance), &pct(1.0 - slow), &num(secs)],
        ),
        data::ItemEffect::Execute(t) => trf(
            "Your hits finish any foe but a boss they leave under {} health.",
            &[&pct(t)],
        ),
        data::ItemEffect::Dodge(c) => trf("{} chance to dodge a blow.", &[&pct(c)]),
        data::ItemEffect::KillShield { amount, max } => trf(
            "Each kill shields you for {}, up to {}.",
            &[&num(amount), &num(max)],
        ),
        data::ItemEffect::Nova {
            every,
            damage,
            radius,
            ..
        } => trf(
            "Every {} sec, a nova strikes everything within {} yd for {} damage.",
            &[&num(every), &num(radius), &num(damage)],
        ),
        data::ItemEffect::Bounty { chance, gold } => trf(
            "{} chance on a kill to find {} more gold.",
            &[&pct(chance), &num(gold)],
        ),
    }
}

/// The `GameTooltip` frame: `UI-Tooltip-Background` tiled under `UI-Tooltip-Border`, 16-unit edges
/// and 5-unit insets, its lines 10 units in. Returns the frame and its line column.
pub(super) fn spawn_tooltip(
    commands: &mut Commands,
    assets: &mut benilla_assets::WorldAssets,
    images: &mut Assets<Image>,
    ui_scale: f32,
) -> (Entity, Entity) {
    use crate::glue::backdrop::{backdrop_border, backdrop_edges, tiled_bg_node};
    let bg = assets.sprite_texture("Interface\\Tooltips\\UI-Tooltip-Background", images);
    let edges = backdrop_edges(assets, "Interface\\Tooltips\\UI-Tooltip-Border", images);
    let mut body = Entity::PLACEHOLDER;
    let mut root = commands.spawn((
        GameTooltip,
        Node {
            position_type: PositionType::Absolute,
            max_width: Val::Px(300.0),
            ..default()
        },
        GlobalZIndex(1000),
        Visibility::Hidden,
        bevy::ui::FocusPolicy::Pass,
    ));
    if bg.is_none() || edges.is_none() {
        root.insert(BackgroundColor(TIP_FILL.with_alpha(0.95)));
    }
    root.with_children(|t| {
        if let Some(bg) = bg {
            t.spawn((
                tiled_bg_node(bg, 16.0, ui_scale, TIP_FILL),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(5.0),
                    right: Val::Px(5.0),
                    top: Val::Px(5.0),
                    bottom: Val::Px(5.0),
                    ..default()
                },
            ));
        }
        if let Some(edges) = &edges {
            backdrop_border(t, edges, 16.0, Color::WHITE);
        }
        body = t
            .spawn((
                TipBody,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    padding: UiRect::all(Val::Px(10.0)),
                    ..default()
                },
            ))
            .id();
    });
    (root.id(), body)
}

/// Hovering a build icon raises its `GameTooltip` under the icon, every number from the current
/// rank and stats, and what the next rank brings.
#[allow(clippy::too_many_arguments)]
pub(super) fn icon_tooltip(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    run: Res<Run>,
    spells: Option<Res<crate::ui_action::Spells>>,
    font: Res<HudFont>,
    ui_scale: Res<UiScale>,
    mut assets: Option<ResMut<benilla_assets::WorldAssets>>,
    mut images: ResMut<Assets<Image>>,
    slots: Query<(
        &Interaction,
        &TipFor,
        &ComputedNode,
        &bevy::ui::UiGlobalTransform,
    )>,
    mut frames: Query<(&mut Node, &mut Visibility), With<GameTooltip>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut built: Local<Option<(Entity, Entity)>>,
    mut shown: Local<Option<(TipFor, u32, u32)>>,
) {
    let Some((frame, body)) = *built else {
        if let Some(assets) = assets.as_deref_mut() {
            *built = Some(spawn_tooltip(
                &mut commands,
                assets,
                &mut images,
                ui_scale.0,
            ));
        }
        return;
    };
    let Ok((mut node, mut vis)) = frames.get_mut(frame) else {
        return;
    };
    let hovered = slots
        .iter()
        .find(|(i, ..)| **i != Interaction::None)
        .filter(|_| *phase.get() != Phase::Menu);
    let Some((_, &tip, computed, at)) = hovered else {
        if *vis != Visibility::Hidden {
            *vis = Visibility::Hidden;
        }
        *shown = None;
        return;
    };
    // Physical pixels to the 768-unit canvas the `Val`s are on. Under the icon, or over it for an
    // icon in the screen's lower half (the action bar, the shop), where under is off the screen.
    let inv = computed.inverse_scale_factor;
    let half = computed.size() / 2.0;
    let screen = windows
        .single()
        .map_or(f32::MAX, |w| w.physical_height() as f32);
    let left = Val::Px((at.translation.x - half.x) * inv);
    let (top, bottom) = if at.translation.y > screen / 2.0 {
        (
            Val::Auto,
            Val::Px((screen - (at.translation.y - half.y)) * inv + 4.0),
        )
    } else {
        (Val::Px((at.translation.y + half.y) * inv + 4.0), Val::Auto)
    };
    if node.left != left || node.top != top || node.bottom != bottom {
        node.left = left;
        node.top = top;
        node.bottom = bottom;
    }
    // The gold in hand changes what a ware's tooltip says.
    let seen = (tip, run.version, run.gold.floor() as u32);
    if *shown == Some(seen) {
        if *vis != Visibility::Inherited {
            *vis = Visibility::Inherited;
        }
        return;
    }
    let stats = run.stats();
    let lines = match tip {
        TipFor::Ability(i) => run
            .abilities
            .get(i)
            .map(|o| ability_tip(o, &stats, spells.as_deref())),
        TipFor::Passive(i) => run
            .passives
            .get(i)
            .map(|&(p, level, power)| passive_tip(p, level, power, &stats)),
        TipFor::RunItem(i) => run
            .run_items
            .get(i)
            .map(|&(item, n)| item_tip(item, n, &stats)),
        TipFor::Ware(i) => run.shop.get(i).map(|w| ware_tip(w, &run, &stats)),
        TipFor::Mobility => run.mobility().map(|m| {
            vec![
                TipLine::new(spell_name(spells.as_deref(), m.spell, m.name), Color::WHITE).header(),
                TipLine::new("Press F", Color::WHITE)
                    .right(format!("{} sec cooldown", num(m.cooldown)), Color::WHITE),
                TipLine::new(m.desc, GOLD),
            ]
        }),
    };
    let Some(lines) = lines else { return };
    *shown = Some(seen);
    // Shown from the next frame, once the new lines have laid out.
    *vis = Visibility::Hidden;
    fill_tooltip(&mut commands, body, &font.0, lines);
}

/// Replace a tooltip body's lines: each a left text and, if it has one, a right-aligned one.
pub(super) fn fill_tooltip(
    commands: &mut Commands,
    body: Entity,
    f: &Handle<Font>,
    lines: Vec<TipLine>,
) {
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        for line in lines {
            let face = |s: String, size: f32, color: Color| {
                (
                    Text::new(s),
                    TextFont {
                        font: f.clone(),
                        font_size: size,
                        ..default()
                    },
                    TextColor(color),
                    TextShadow {
                        offset: Vec2::splat(1.0),
                        color: Color::BLACK,
                    },
                )
            };
            b.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(16.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(face(line.left, line.size, line.color));
                if let Some((text, color)) = line.right {
                    row.spawn((
                        face(text, line.size, color),
                        TextLayout::new(Justify::Right, LineBreak::NoWrap),
                    ));
                }
            });
        }
    });
}

#[allow(clippy::type_complexity)]
pub(super) fn show_panels(
    phase: Res<State<Phase>>,
    sheet: Option<Res<super::sheet::SheetOpen>>,
    mut roots: ParamSet<(
        Query<(&mut Visibility, &mut Node), With<MenuRoot>>,
        Query<(&mut Visibility, &mut Node), With<HudRoot>>,
        Query<(&mut Visibility, &mut Node), With<LevelUpRoot>>,
        Query<(&mut Visibility, &mut Node), With<PauseRoot>>,
        Query<(&mut Visibility, &mut Node), With<OverRoot>>,
    )>,
) {
    let p = *phase.get();
    // A hidden panel leaves the layout too: a merely invisible button still takes clicks.
    let set = |(mut v, mut node): (Mut<Visibility>, Mut<Node>), on: bool| {
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
        let display = if on { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
    };
    for root in &mut roots.p0() {
        // The new main screen (`character`) takes the glue menu's place when it is on.
        set(root, p == Phase::Menu && !super::character::enabled());
    }
    for root in &mut roots.p1() {
        set(
            root,
            !matches!(p, Phase::Menu | Phase::GameOver | Phase::Lobby),
        );
    }
    for root in &mut roots.p2() {
        set(root, p == Phase::LevelUp);
    }
    // The character sheet pauses the run too, without the pause screen.
    let sheet = sheet.is_some_and(|s| s.0);
    for root in &mut roots.p3() {
        set(root, p == Phase::Paused && !sheet);
    }
    for root in &mut roots.p4() {
        set(root, p == Phase::GameOver);
    }
}
