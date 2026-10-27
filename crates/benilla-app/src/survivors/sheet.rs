//! The character sheet, as WoW's `CharacterFrame` lays it out: `C` in a run opens
//! `C`, Escape or its close button shut it. The hero's name and "Level N Race Class" on
//! top; the paper doll's slots down the sides and along the bottom around the live hero, which the
//! camera frames into the sheet's stage ([`super::hero::StageFrame`]) and a drag or the arrows
//! turn; the hero's numbers under it, and what its run items do beside it.
//!
//! The armour and weapon slots show the gear the profile has on, else the class's starting
//! outfit, else the paper doll's empty art. The neck, the two rings and the two trinkets are the
//! run's items ([`super::Vitals::run_items`], five kinds at most, [`data::MAX_RUN_ITEM_KINDS`]),
//! each with its stacks.

use bevy::prelude::*;

use super::character::{bonus_text, gear_tip};
use super::data::{self, GearSlot, ItemEffect};
use super::hero::{CamRig, StageDrag, StageFrame};
use super::hud::{self, num, pct, TipLine};
use super::kit::{self, theme, IconSource, KitClicks, KitFonts, Tooltip};
use super::lang::{tr, trf};
use super::profile::ProfileStore;
use super::{menu, outfit, Phase, Run, Selection};

/// The sheet is open: the run is paused under it, and the pause screen stays hidden.
#[derive(Resource, Default)]
pub(super) struct SheetOpen(pub bool);

#[derive(Component)]
struct SheetRoot;

/// The parts rebuilt when the hero changes.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum SheetPart {
    Title,
    Left,
    Right,
    Bottom,
    Stats,
    Side,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum SheetAction {
    Close,
    Turn(i8),
}

/// A paper-doll slot: a piece of gear (its [`GearSlot::ALL`] index), a piece only the starting
/// outfit fills (its equipment slot, its name, its empty art), or one of the run's items.
#[derive(Clone, Copy)]
enum Slot {
    Gear(usize),
    Outfit(u8, &'static str, &'static str),
    Run(usize, &'static str, &'static str),
}

/// The left column, top to bottom, as WoW's: head, neck, shoulders, back, chest, shirt, tabard,
/// wrists.
const LEFT: [Slot; 8] = [
    Slot::Gear(0),
    Slot::Run(0, "Neck", "Neck"),
    Slot::Gear(1),
    Slot::Outfit(outfit::slot::BACK, "Cloak", "Chest"),
    Slot::Gear(2),
    Slot::Outfit(outfit::slot::SHIRT, "Shirt", "Shirt"),
    Slot::Outfit(outfit::slot::TABARD, "Tabard", "Tabard"),
    Slot::Gear(3),
];
/// The right column: hands, waist, legs, feet, two rings, two trinkets.
const RIGHT: [Slot; 8] = [
    Slot::Gear(4),
    Slot::Gear(5),
    Slot::Gear(6),
    Slot::Gear(7),
    Slot::Run(1, "Finger", "Finger"),
    Slot::Run(2, "Finger", "Finger"),
    Slot::Run(3, "Trinket", "Trinket"),
    Slot::Run(4, "Trinket", "Trinket"),
];
/// The bottom row: main hand, off hand, ranged.
const BOTTOM: [Slot; 3] = [
    Slot::Gear(8),
    Slot::Gear(9),
    Slot::Outfit(outfit::slot::RANGED, "Ranged", "Ranged"),
];

/// The slots' icon size.
const SLOT: f32 = 38.0;
/// The stage's height.
const STAGE_HEIGHT: f32 = 300.0;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SheetOpen>()
        .add_systems(
            Update,
            (build_sheet, show_sheet, sheet_clicks, refresh_sheet)
                .chain()
                .after(hud::show_panels),
        )
        // However the pause ends (the sheet's own keys, the main menu, a run's end), it shuts.
        .add_systems(OnExit(Phase::Paused), close_sheet);
}

fn close_sheet(mut open: ResMut<SheetOpen>) {
    open.0 = false;
}

/// Shown while open; opened with `C` by `hud::keys`, which pauses the run under it.
fn show_sheet(
    open: Res<SheetOpen>,
    mut roots: Query<(&mut Visibility, &mut Node), With<SheetRoot>>,
) {
    for (mut v, mut n) in &mut roots {
        let (vis, display) = if open.0 {
            (Visibility::Inherited, Display::Flex)
        } else {
            (Visibility::Hidden, Display::None)
        };
        if *v != vis {
            *v = vis;
        }
        if n.display != display {
            n.display = display;
        }
    }
}

fn sheet_clicks(
    clicks: Res<KitClicks>,
    buttons: Query<(Entity, &SheetAction)>,
    mut open: ResMut<SheetOpen>,
    mut rig: ResMut<CamRig>,
    mut next: ResMut<NextState<Phase>>,
) {
    match clicks.first(&buttons) {
        Some(SheetAction::Close) => {
            open.0 = false;
            next.set(Phase::Playing);
        }
        Some(SheetAction::Turn(step)) => rig.orbit += f32::from(*step) * 0.5,
        None => {}
    }
}

/// A piece of the sheet's fill. The stage has none, so the hero framed in it shows through.
fn fill() -> BackgroundColor {
    BackgroundColor(theme::PANEL_TOP)
}

fn build_sheet(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    built: Query<(), With<SheetRoot>>,
) {
    let Some(fonts) = fonts else { return };
    if !built.is_empty() {
        return;
    }
    let f = &*fonts;
    let part = |p: SheetPart, node: Node| (p, node);
    commands
        .spawn((
            SheetRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(40.0),
                top: Val::Px(60.0),
                column_gap: Val::Px(6.0),
                align_items: AlignItems::FlexStart,
                display: Display::None,
                ..default()
            },
            GlobalZIndex(430),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(6.0)),
                    ..default()
                },
                BorderColor::all(theme::BRASS),
            ))
            .with_children(|main| {
                // The title: the name, the level, race and class, and the close button.
                main.spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(12.0), Val::Px(8.0)),
                        justify_content: JustifyContent::Center,
                        border_radius: BorderRadius::top(Val::Px(6.0)),
                        ..default()
                    },
                    fill(),
                ))
                .with_children(|t| {
                    t.spawn(part(
                        SheetPart::Title,
                        Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                    ));
                    t.spawn(Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(8.0),
                        top: Val::Px(8.0),
                        ..default()
                    })
                    .with_children(|c| {
                        kit::button(c, f, "X", None, kit::ButtonLook::ACTION, SheetAction::Close);
                    });
                });
                main.spawn(Node::default()).with_children(|body| {
                    let column = |p: SheetPart| {
                        (
                            p,
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(5.0),
                                padding: UiRect::all(Val::Px(8.0)),
                                ..default()
                            },
                            fill(),
                        )
                    };
                    body.spawn(column(SheetPart::Left));
                    body.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(330.0),
                        ..default()
                    })
                    .with_children(|mid| {
                        // The stage: no fill, a drag turns the hero; the arrows turn it too.
                        mid.spawn((
                            StageFrame { turning: false },
                            StageDrag,
                            Button,
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(STAGE_HEIGHT),
                                border: UiRect::all(Val::Px(1.0)),
                                ..default()
                            },
                            BorderColor::all(theme::BRASS.with_alpha(0.5)),
                        ))
                        .with_children(|stage| {
                            stage
                                .spawn(Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Px(8.0),
                                    top: Val::Px(8.0),
                                    column_gap: Val::Px(8.0),
                                    ..default()
                                })
                                .with_children(|r| {
                                    for (art, step) in [("Left", -1), ("Right", 1)] {
                                        kit::icon_button(
                                            r,
                                            f,
                                            IconSource::Texture(format!(
                                                "Interface\\Buttons\\UI-Rotation{art}-Button-Up"
                                            )),
                                            22.0,
                                            None,
                                            false,
                                            SheetAction::Turn(step),
                                        );
                                    }
                                });
                        });
                        mid.spawn((
                            SheetPart::Stats,
                            Node {
                                column_gap: Val::Px(6.0),
                                padding: UiRect::all(Val::Px(6.0)),
                                ..default()
                            },
                            fill(),
                        ));
                    });
                    body.spawn(column(SheetPart::Right));
                });
                main.spawn((
                    SheetPart::Bottom,
                    Node {
                        justify_content: JustifyContent::Center,
                        column_gap: Val::Px(6.0),
                        padding: UiRect::all(Val::Px(8.0)),
                        border_radius: BorderRadius::bottom(Val::Px(6.0)),
                        ..default()
                    },
                    fill(),
                ));
            });
            // Beside it, what the run's items do.
            kit::panel(
                root,
                f,
                None,
                Node {
                    width: Val::Px(230.0),
                    ..default()
                },
                |p| {
                    p.spawn((
                        SheetPart::Side,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(4.0),
                            ..default()
                        },
                    ));
                },
            );
        });
}

/// What the sheet shows, to rebuild it only when that changes.
#[derive(PartialEq, Clone, Debug)]
struct SheetView {
    version: u32,
    level: u32,
    items: Vec<(&'static str, u8)>,
    gold: i64,
    gear: Vec<String>,
    look: Option<(u8, u8)>,
    ready: (bool, bool, bool),
    lang: u32,
}

/// A slot's icon with its tooltip, a coloured rim for what fills it, and a stack count.
fn slot_icon(
    p: &mut ChildSpawnerCommands,
    f: &KitFonts,
    icon: IconSource,
    tip: Tooltip,
    rim: Option<Color>,
    count: Option<u8>,
) {
    let e = kit::icon(p, icon, SLOT, Some(tip));
    if let Some(rim) = rim {
        p.commands().entity(e).insert(BorderColor::all(rim));
    }
    if let Some(n) = count.filter(|&n| n > 1) {
        p.commands().entity(e).with_children(|c| {
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(2.0),
                    bottom: Val::Px(0.0),
                    ..default()
                },
                kit::text(&f.number, n.to_string(), 13.0, Color::WHITE),
            ));
        });
    }
}

/// One labelled number of a stat box.
fn stat_line(p: &mut ChildSpawnerCommands, f: &KitFonts, label: &str, value: String) {
    p.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        column_gap: Val::Px(12.0),
        ..default()
    })
    .with_children(|r| {
        r.spawn(kit::text(&f.body, label, 14.0, theme::GOLD));
        r.spawn(kit::text(&f.number, value, 14.0, Color::WHITE));
    });
}

/// A box of stat lines.
fn stat_box(p: &mut ChildSpawnerCommands, lines: impl FnOnce(&mut ChildSpawnerCommands)) {
    p.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
            row_gap: Val::Px(2.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme::INSET),
        BorderColor::all(theme::BRASS.with_alpha(0.6)),
    ))
    .with_children(lines);
}

/// "+25%" for a multiplier of 1.25, "-10%" for 0.9.
fn signed_pct(mult: f32) -> String {
    let v = (mult - 1.0) * 100.0;
    if v >= 0.0 {
        format!("+{}%", num(v))
    } else {
        format!("{}%", num(v))
    }
}

/// What the run's items do beyond their stats, added up: lifesteal and kill heal as amounts,
/// thorns as damage, a chain's and a burst's best chance.
#[derive(Default, Debug, PartialEq)]
struct Effects {
    lifesteal: f32,
    kill_heal: f32,
    thorns: f32,
    chain: f32,
    burst: f32,
    ignite: f32,
    chill: f32,
    execute: f32,
    dodge: f32,
    kill_shield: f32,
    nova: f32,
    bounty: f32,
}

fn effects(all: impl Iterator<Item = ItemEffect>) -> Effects {
    let mut e = Effects::default();
    for x in all {
        match x {
            ItemEffect::Lifesteal(v) => e.lifesteal += v,
            ItemEffect::KillHeal(v) => e.kill_heal += v,
            ItemEffect::Thorns(v) => e.thorns += v,
            ItemEffect::Chain { chance, .. } => e.chain = e.chain.max(chance),
            ItemEffect::Burst { chance, .. } => e.burst = e.burst.max(chance),
            ItemEffect::Ignite { chance, .. } => e.ignite = e.ignite.max(chance),
            ItemEffect::Chill { chance, .. } => e.chill = e.chill.max(chance),
            ItemEffect::Execute(t) => e.execute = e.execute.max(t),
            ItemEffect::Dodge(c) => e.dodge = (e.dodge + c).min(data::MAX_DODGE),
            ItemEffect::KillShield { amount, .. } => e.kill_shield += amount,
            ItemEffect::Nova { damage, .. } => e.nova = e.nova.max(damage),
            ItemEffect::Bounty { chance, .. } => e.bounty = e.bounty.max(chance),
        }
    }
    e
}

#[allow(clippy::too_many_arguments)]
fn refresh_sheet(
    mut commands: Commands,
    open: Res<SheetOpen>,
    fonts: Option<Res<KitFonts>>,
    run: Res<Run>,
    selection: Res<Selection>,
    store: Res<ProfileStore>,
    catalog: Option<Res<crate::entities::CharCreate>>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    parts: Query<(Entity, &SheetPart)>,
    mut shown: Local<Option<SheetView>>,
    mut lang_seen: Local<u32>,
) {
    let Some(fonts) = fonts else { return };
    if !open.0 {
        return;
    }
    let look = menu::current_look(&selection);
    let gear = store.profile.gear();
    super::lang::stale(&mut lang_seen);
    let view = SheetView {
        version: run.version,
        level: run.level,
        items: run.run_items.iter().map(|(i, n)| (i.key, *n)).collect(),
        gold: run.gold.round() as i64,
        gear: gear.iter().map(|i| i.name.clone()).collect(),
        look,
        ready: (catalog.is_some(), displays.is_some(), spells.is_some()),
        lang: *lang_seen,
    };
    if shown.as_ref() == Some(&view) {
        return;
    }
    let part = |want: SheetPart| parts.iter().find(|(_, p)| **p == want).map(|(e, _)| e);
    let (Some(title), Some(left), Some(right), Some(bottom), Some(stats_box), Some(side)) = (
        part(SheetPart::Title),
        part(SheetPart::Left),
        part(SheetPart::Right),
        part(SheetPart::Bottom),
        part(SheetPart::Stats),
        part(SheetPart::Side),
    ) else {
        return;
    };
    *shown = Some(view);
    let f = &*fonts;
    let class = run.class;
    let stats = run.stats();
    for e in [title, left, right, bottom, stats_box, side] {
        commands.entity(e).despawn_related::<Children>();
    }

    commands.entity(title).with_children(|t| {
        t.spawn(kit::text(
            &f.body,
            store.profile.hero.name.clone(),
            20.0,
            Color::WHITE,
        ));
        let level = run.level.to_string();
        let race = crate::char_select::race_name(run.race);
        t.spawn(kit::text(
            &f.body,
            trf("Level {} {} {}", &[&level, race, tr(class.name)]),
            15.0,
            theme::GOLD,
        ));
    });

    // The starting outfit, for the slots no gear fills.
    let worn = match look {
        Some((race, sex)) => match outfit::body(
            class,
            race,
            sex,
            selection.appearance,
            0,
            &[],
            catalog.as_deref().map(|c| &c.0),
            None,
        ) {
            outfit::Body::Player { worn, .. } => worn,
            outfit::Body::Trainer { .. } => Vec::new(),
        },
        None => Vec::new(),
    };
    let outfit_icon = |slot: u8| -> Option<String> {
        let piece = worn.iter().find(|w| w.slot == slot)?;
        displays
            .as_deref()?
            .catalog
            .get(piece.display)?
            .icon
            .clone()
    };
    let empty = |art: &str| format!("Interface\\PaperDoll\\UI-PaperDoll-Slot-{art}");
    let fill_slot = |p: &mut ChildSpawnerCommands, slot: Slot| match slot {
        Slot::Gear(k) => {
            let (label, art, outfit_slot) = outfit::SHOWN[k];
            let gear_slot = GearSlot::ALL[k];
            match gear.iter().find(|i| i.slot == gear_slot) {
                Some(item) => {
                    let icon = displays
                        .as_deref()
                        .and_then(|d| outfit::gear_icon(item, Some(class), &d.catalog))
                        .unwrap_or_else(|| empty(art));
                    slot_icon(
                        p,
                        f,
                        IconSource::Texture(icon),
                        gear_tip(item, tr("Worn this run").to_string()),
                        Some(hud::rgb(item.rarity.color())),
                        None,
                    );
                }
                None => {
                    let start = outfit_icon(outfit_slot);
                    let note = if start.is_some() {
                        TipLine::new(tr("Starting gear"), theme::TEXT)
                    } else {
                        TipLine::new(tr("Empty"), theme::DIM)
                    };
                    slot_icon(
                        p,
                        f,
                        IconSource::Texture(start.unwrap_or_else(|| empty(art))),
                        Tooltip(vec![TipLine::new(tr(label), Color::WHITE).header(), note]),
                        None,
                        None,
                    );
                }
            }
        }
        Slot::Outfit(s, label, art) => {
            let start = outfit_icon(s);
            let note = if start.is_some() {
                TipLine::new(tr("Starting gear"), theme::TEXT)
            } else {
                TipLine::new(tr("Empty"), theme::DIM)
            };
            slot_icon(
                p,
                f,
                IconSource::Texture(start.unwrap_or_else(|| empty(art))),
                Tooltip(vec![TipLine::new(tr(label), Color::WHITE).header(), note]),
                None,
                None,
            );
        }
        Slot::Run(i, label, art) => match run.run_items.get(i) {
            Some(&(item, n)) => slot_icon(
                p,
                f,
                IconSource::Spell(item.spell),
                Tooltip(hud::item_tip(item, n, &stats)),
                Some(super::cards::rarity_color(item.rarity)),
                Some(n),
            ),
            None => slot_icon(
                p,
                f,
                IconSource::Texture(empty(art)),
                Tooltip(vec![
                    TipLine::new(tr(label), Color::WHITE).header(),
                    TipLine::new(tr("A run item goes here"), theme::DIM),
                ]),
                None,
                None,
            ),
        },
    };
    for (e, slots) in [
        (left, &LEFT[..]),
        (right, &RIGHT[..]),
        (bottom, &BOTTOM[..]),
    ] {
        commands.entity(e).with_children(|p| {
            for &s in slots {
                fill_slot(p, s);
            }
        });
    }

    commands.entity(stats_box).with_children(|row| {
        stat_box(row, |b| {
            stat_line(b, f, tr("Health"), num(stats.max_hp.round()));
            stat_line(b, f, tr("Damage"), signed_pct(stats.might));
            stat_line(b, f, tr("Haste"), format!("+{}", pct(stats.haste)));
            stat_line(b, f, tr("Crit"), pct(stats.crit));
            stat_line(b, f, tr("Armor"), num(stats.armor));
            stat_line(b, f, tr("Reduction"), pct(stats.damage_reduction));
        });
        stat_box(row, |b| {
            stat_line(b, f, tr("Area"), signed_pct(stats.area));
            stat_line(
                b,
                f,
                tr("Speed"),
                signed_pct(stats.speed / data::Stat::Speed.base()),
            );
            stat_line(b, f, tr("Regen"), num(stats.regen));
            stat_line(b, f, tr("Pickup"), format!("{} yd", num(stats.magnet)));
        });
    });

    let fx = effects(run.item_effects());
    commands.entity(side).with_children(|p| {
        p.spawn(kit::text(&f.title, tr("This run"), 18.0, theme::GOLD));
        stat_line(p, f, tr("Experience"), signed_pct(stats.growth));
        stat_line(p, f, tr("Gold"), signed_pct(stats.gold));
        stat_line(p, f, tr("Gold in hand"), num(run.gold.round()));
        stat_line(p, f, tr("Lifesteal"), pct(fx.lifesteal));
        stat_line(p, f, tr("Kill Heal"), num(fx.kill_heal));
        stat_line(p, f, tr("Thorns"), num(fx.thorns * stats.might));
        stat_line(p, f, tr("Chain"), pct(fx.chain));
        stat_line(p, f, tr("Burst"), pct(fx.burst));
        // The rarer effects, only once an item brings them.
        let more = [
            ("Ignite", fx.ignite, pct(fx.ignite)),
            ("Chill", fx.chill, pct(fx.chill)),
            ("Execute", fx.execute, pct(fx.execute)),
            ("Dodge", fx.dodge, pct(fx.dodge)),
            ("Kill Shield", fx.kill_shield, num(fx.kill_shield)),
            ("Nova", fx.nova, num(fx.nova * stats.might)),
            ("Bounty", fx.bounty, pct(fx.bounty)),
        ];
        for (label, v, shown) in more {
            if v > 0.0 {
                stat_line(p, f, tr(label), shown);
            }
        }
        stat_line(
            p,
            f,
            tr("Run items"),
            format!("{} / {}", run.run_items.len(), data::MAX_RUN_ITEM_KINDS),
        );
        // What the gear worn gives, if anything.
        let bonuses: Vec<String> = data::sum_bonuses(
            &gear
                .iter()
                .flat_map(|i| i.bonuses.iter().cloned())
                .collect::<Vec<_>>(),
        )
        .iter()
        .map(|b| format!("+{}", bonus_text(b)))
        .collect();
        if !bonuses.is_empty() {
            p.spawn(kit::text(&f.title, tr("Gear"), 16.0, theme::GOLD));
            p.spawn(kit::text(&f.body, bonuses.join(" · "), 13.0, theme::GREEN));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paper_doll_holds_every_gear_slot_and_five_run_items() {
        let mut gear: Vec<usize> = Vec::new();
        let mut run = Vec::new();
        for s in LEFT.iter().chain(&RIGHT).chain(&BOTTOM) {
            match *s {
                Slot::Gear(k) => gear.push(k),
                Slot::Run(i, ..) => run.push(i),
                Slot::Outfit(..) => {}
            }
        }
        gear.sort_unstable();
        assert_eq!(gear, (0..GearSlot::ALL.len()).collect::<Vec<_>>());
        run.sort_unstable();
        assert_eq!(run, (0..data::MAX_RUN_ITEM_KINDS).collect::<Vec<_>>());
    }

    #[test]
    fn the_items_effects_add_up() {
        let e = effects(
            [
                ItemEffect::Lifesteal(0.02),
                ItemEffect::Lifesteal(0.03),
                ItemEffect::KillHeal(2.0),
                ItemEffect::Thorns(4.0),
                ItemEffect::Dodge(0.3),
                ItemEffect::Dodge(0.3),
                ItemEffect::Execute(0.05),
                ItemEffect::Execute(0.1),
            ]
            .into_iter(),
        );
        assert_eq!(e.dodge, data::MAX_DODGE);
        assert_eq!(e.execute, 0.1);
        assert!((e.lifesteal - 0.05).abs() < 1e-6);
        assert_eq!(e.kill_heal, 2.0);
        assert_eq!(e.thorns, 4.0);
        assert_eq!(signed_pct(1.25), "+25%");
        assert_eq!(signed_pct(0.9), "-10%");
    }
}
