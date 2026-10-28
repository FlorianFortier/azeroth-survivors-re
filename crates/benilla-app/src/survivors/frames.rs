//! The in-run HUD's WoW frames, after the reference screen:
//!
//! - the **player frame** at the top left: `PlayerFrame`'s art, the race's portrait, the hero's
//!   name, health and power bars, and the level in its seat;
//! - the **target frame** beside it: `TargetFrame`'s art, the target's name on its red bar, its
//!   health and level, shown while the hero has a target;
//! - the **damage meter** under the minimap: the run's damage by spell, a bar each, the top first;
//! - the **action bar** at the bottom: `MainMenuBar`'s art and gryphons under the twelve slots the
//!   hero's spells and its movement spell sit in, the experience bar on top.
//!
//! The run's own lines (the clock, the next boss, kills, gold, the chest's price, the keys) stay
//! where the HUD has them. Everything here reads [`Run`] and the replicated units, so a guest's
//! frames show what its host's do.

use bevy::prelude::*;

use super::data;
use super::enemies::Enemy;
use super::hud::{self, HudFont, SpellIcon, UiArt};
use super::lang::{tr, trf};
use super::targeting::HeroTarget;
use super::treasure::FieldMark;
use super::{Run, Selection, Session};
use crate::net::ObjectStore;

/// `UI-TargetingFrame`'s cut of the frame both unit frames wear; the player's is it mirrored.
const FRAME_ART: &str = "Interface\\TargetingFrame\\UI-TargetingFrame";
const FRAME_COORDS: [f32; 4] = [0.09375, 1.0, 0.0, 0.781_25];
const BAR_ART: &str = "Interface\\TargetingFrame\\UI-StatusBar";
const NAME_BG_ART: &str = "Interface\\TargetingFrame\\UI-TargetingFrame-LevelBackground";
const FRAME_W: f32 = 232.0;
const FRAME_H: f32 = 100.0;
const PORTRAIT: f32 = 64.0;
const BAR_W: f32 = 119.0;
const BAR_H: f32 = 12.0;

/// The action bar's art: four 256-wide cuts of `UI-MainMenuBar-Dwarf`, and its gryphons.
const BAR_ART_MAIN: &str = "Interface\\MainMenuBar\\UI-MainMenuBar-Dwarf";
const BAR_CUTS: [f32; 4] = [0.832_031_25, 0.582_031_25, 0.332_031_25, 0.082_031_25];
const END_CAP: &str = "Interface\\MainMenuBar\\UI-MainMenuBar-EndCap-Dwarf";
const QUICKSLOT: &str = "Interface\\Buttons\\UI-Quickslot2";
/// `ActionButton1` at (8, 4) from the bar's bottom left, each 36 wide and 42 apart.
pub(super) const SLOT: f32 = 36.0;
pub(super) const SLOT_STEP: f32 = 42.0;
pub(super) const SLOT_LEFT: f32 = 8.0;
pub(super) const SLOT_BOTTOM: f32 = 4.0;
/// The action bar's height: its art; the experience bar sits on it.
pub(super) const BAR_TALL: f32 = 43.0;

/// How many spells the damage meter lists.
const METER_ROWS: usize = 6;

#[derive(Component)]
pub(super) struct PlayerName;
#[derive(Component)]
pub(super) struct PlayerHealth;
#[derive(Component)]
pub(super) struct PlayerHealthText;
#[derive(Component)]
pub(super) struct PlayerPower;
#[derive(Component)]
pub(super) struct PlayerLevel;
#[derive(Component)]
pub(super) struct PlayerPortrait;
#[derive(Component)]
pub(super) struct TargetFrame;
#[derive(Component)]
pub(super) struct TargetName;
#[derive(Component)]
pub(super) struct TargetHealth;
#[derive(Component)]
pub(super) struct TargetLevel;
#[derive(Component)]
pub(super) struct Meter;
#[derive(Component)]
pub(super) struct MeterRows;

fn unpainted() -> ImageNode {
    ImageNode {
        color: Color::NONE,
        ..default()
    }
}

fn at(left: f32, top: f32, w: f32, h: f32) -> Node {
    Node {
        position_type: PositionType::Absolute,
        left: Val::Px(left),
        top: Val::Px(top),
        width: Val::Px(w),
        height: Val::Px(h),
        ..default()
    }
}

fn label(font: &Handle<Font>, s: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: font.clone(),
            font_size: size,
            ..default()
        },
        TextColor(color),
        TextShadow {
            offset: Vec2::splat(1.0),
            color: Color::BLACK,
        },
        TextLayout::new(Justify::Center, bevy::text::LineBreak::NoWrap),
    )
}

/// A bar: a dark well, its fill in `UI-StatusBar` tinted `color`.
fn bar(
    parent: &mut ChildSpawnerCommands,
    left: f32,
    top: f32,
    color: Color,
    fill: impl Component,
) -> Entity {
    let mut id = Entity::PLACEHOLDER;
    parent
        .spawn((
            at(left, top, BAR_W, BAR_H),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ))
        .with_children(|b| {
            id = b
                .spawn((
                    fill,
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    unpainted(),
                    UiArt::tinted(BAR_ART, color),
                ))
                .id();
        });
    id
}

/// The race's portrait for the player frame: `TemporaryPortrait-<Sex>-<Race>`.
fn portrait_of(race: u8, sex: u8) -> &'static str {
    const MALE: [&str; 8] = [
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-Human",
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-Orc",
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-Dwarf",
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-NightElf",
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-Scourge",
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-Tauren",
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-Gnome",
        "Interface\\CharacterFrame\\TemporaryPortrait-Male-Troll",
    ];
    const FEMALE: [&str; 8] = [
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-Human",
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-Orc",
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-Dwarf",
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-NightElf",
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-Scourge",
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-Tauren",
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-Gnome",
        "Interface\\CharacterFrame\\TemporaryPortrait-Female-Troll",
    ];
    let i = usize::from(race.clamp(1, 8) - 1);
    if sex == 1 {
        FEMALE[i]
    } else {
        MALE[i]
    }
}

/// The power bar's colour for the class's power: mana, rage, energy.
pub(super) fn power_color(power: u8) -> Color {
    match power {
        1 => Color::srgb(1.0, 0.0, 0.0),
        3 => Color::srgb(1.0, 1.0, 0.0),
        _ => Color::srgb(0.0, 0.0, 1.0),
    }
}

/// The unit frames and the damage meter, under the HUD's root.
pub(super) fn spawn_frames(root: &mut ChildSpawnerCommands, f: &Handle<Font>) {
    // The player frame, `PlayerFrame` at (-19, -4): the mirrored frame over the portrait and bars.
    root.spawn(at(-19.0, 4.0, FRAME_W, FRAME_H))
        .with_children(|p| {
            p.spawn((
                PlayerPortrait,
                Node {
                    border_radius: BorderRadius::MAX,
                    ..at(42.0, 12.0, PORTRAIT, PORTRAIT)
                },
                BackgroundColor(Color::BLACK),
                unpainted(),
            ));
            bar(p, 106.0, 41.0, Color::srgb(0.0, 1.0, 0.0), PlayerHealth);
            bar(p, 106.0, 52.0, power_color(0), PlayerPower);
            p.spawn((
                at(0.0, 0.0, FRAME_W, FRAME_H),
                ImageNode {
                    flip_x: true,
                    ..unpainted()
                },
                UiArt::crop(FRAME_ART, FRAME_COORDS),
            ));
            p.spawn(Node {
                justify_content: JustifyContent::Center,
                ..at(106.0, 23.0, BAR_W, 14.0)
            })
            .with_children(|n| {
                n.spawn((PlayerName, label(f, "", 13.0, Color::srgb(1.0, 0.82, 0.0))));
            });
            p.spawn(Node {
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..at(106.0, 41.0, BAR_W, BAR_H)
            })
            .with_children(|n| {
                n.spawn((PlayerHealthText, label(f, "", 10.0, Color::WHITE)));
            });
            p.spawn(Node {
                justify_content: JustifyContent::Center,
                ..at(40.0, 60.0, 26.0, 14.0)
            })
            .with_children(|n| {
                n.spawn((
                    PlayerLevel,
                    label(f, "1", 12.0, Color::srgb(1.0, 0.82, 0.0)),
                ));
            });
        });

    // The target frame, `TargetFrame` at (250, -4): the frame, its red name bar, health, level.
    root.spawn((
        TargetFrame,
        at(250.0, 4.0, FRAME_W, FRAME_H),
        Visibility::Hidden,
    ))
    .with_children(|t| {
        t.spawn((
            Node {
                border_radius: BorderRadius::MAX,
                ..at(FRAME_W - 42.0 - PORTRAIT, 12.0, PORTRAIT, PORTRAIT)
            },
            BackgroundColor(Color::srgb(0.08, 0.05, 0.04)),
            unpainted(),
            UiArt::crop(
                "Interface\\CharacterFrame\\TemporaryPortrait-Monster",
                [0.0, 1.0, 0.0, 1.0],
            ),
        ));
        t.spawn((
            at(FRAME_W - 106.0 - BAR_W, 22.0, BAR_W, 19.0),
            unpainted(),
            UiArt::tinted(NAME_BG_ART, Color::srgba(1.0, 0.0, 0.0, 0.85)),
        ));
        bar(
            t,
            FRAME_W - 106.0 - BAR_W,
            41.0,
            Color::srgb(0.0, 1.0, 0.0),
            TargetHealth,
        );
        t.spawn((
            at(FRAME_W - 106.0 - BAR_W, 52.0, BAR_W, BAR_H),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        ));
        t.spawn((
            at(0.0, 0.0, FRAME_W, FRAME_H),
            unpainted(),
            UiArt::crop(FRAME_ART, FRAME_COORDS),
        ));
        t.spawn(Node {
            justify_content: JustifyContent::Center,
            ..at(FRAME_W - 106.0 - BAR_W, 24.0, BAR_W, 14.0)
        })
        .with_children(|n| {
            n.spawn((TargetName, label(f, "", 12.0, Color::WHITE)));
        });
        t.spawn(Node {
            justify_content: JustifyContent::Center,
            ..at(FRAME_W - 40.0 - 26.0, 60.0, 26.0, 14.0)
        })
        .with_children(|n| {
            n.spawn((TargetLevel, label(f, "", 12.0, Color::srgb(1.0, 0.82, 0.0))));
        });
    });

    // The damage meter, on the right under the minimap.
    root.spawn((
        Meter,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(6.0),
            top: Val::Px(250.0),
            width: Val::Px(200.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(3.0)),
            row_gap: Val::Px(2.0),
            border: UiRect::all(Val::Px(1.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.55)),
        BorderColor::all(Color::srgba(1.0, 1.0, 1.0, 0.15)),
        Visibility::Hidden,
    ))
    .with_children(|m| {
        m.spawn((
            Node {
                padding: UiRect::axes(Val::Px(3.0), Val::Px(1.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.15, 0.15, 0.2, 0.8)),
        ))
        .with_children(|h| {
            h.spawn(label(f, tr("Damage Done"), 11.0, Color::WHITE));
        });
        m.spawn((
            MeterRows,
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(1.0),
                ..default()
            },
        ));
    });
}

/// The action bar's art and its twelve empty slots, centred at the bottom; the spells' icons and
/// the movement spell's button sit on the slots (`hud.rs`).
pub(super) fn spawn_action_bar(root: &mut ChildSpawnerCommands) {
    root.spawn(Node {
        position_type: PositionType::Absolute,
        left: Val::Percent(50.0),
        bottom: Val::Px(0.0),
        margin: UiRect::left(Val::Px(-512.0)),
        width: Val::Px(1024.0),
        height: Val::Px(BAR_TALL),
        ..default()
    })
    .with_children(|b| {
        // The gryphons, 128 square, three quarters outside the bar's ends.
        b.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(-96.0),
                bottom: Val::Px(0.0),
                width: Val::Px(128.0),
                height: Val::Px(128.0),
                ..default()
            },
            unpainted(),
            UiArt::crop(END_CAP, [0.0, 1.0, 0.0, 1.0]),
        ));
        b.spawn((
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(-96.0),
                bottom: Val::Px(0.0),
                width: Val::Px(128.0),
                height: Val::Px(128.0),
                ..default()
            },
            ImageNode {
                flip_x: true,
                ..unpainted()
            },
            UiArt::crop(END_CAP, [0.0, 1.0, 0.0, 1.0]),
        ));
        for (i, top) in BAR_CUTS.into_iter().enumerate() {
            b.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(256.0 * i as f32),
                    bottom: Val::Px(0.0),
                    width: Val::Px(256.0),
                    height: Val::Px(BAR_TALL),
                    ..default()
                },
                unpainted(),
                UiArt::crop(BAR_ART_MAIN, [0.0, 1.0, top, top + 0.167_968_75]),
            ));
        }
        for i in 0..12 {
            let left = SLOT_LEFT + SLOT_STEP * i as f32;
            b.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(left - SLOT * 0.42),
                    bottom: Val::Px(SLOT_BOTTOM - SLOT * 0.42 - 1.0),
                    width: Val::Px(SLOT * 1.84),
                    height: Val::Px(SLOT * 1.84),
                    ..default()
                },
                unpainted(),
                UiArt::crop(QUICKSLOT, [0.0, 1.0, 0.0, 1.0]),
            ));
        }
    });
}

/// What to call the hero's target: the unit's own name where this machine plays it, else the
/// creature of the zone that wears its look (an elite's kind after it).
fn target_name(
    enemy: Option<&Enemy>,
    display: Option<u32>,
    mark: Option<&FieldMark>,
    zone: usize,
) -> String {
    if let Some(e) = enemy {
        return e.def.name.to_string();
    }
    let Some(d) = display else {
        return String::new();
    };
    let roster = data::roster(zone);
    let base = roster
        .enemies
        .iter()
        .chain(&roster.bosses)
        .find(|e| e.wears(d))
        .map_or("", |e| e.name);
    // The bonus boss goes by its own name.
    if let Some(bonus) = data::bonus_boss(zone).filter(|b| b.wears(d)) {
        if mark == Some(&FieldMark::Elite(data::BONUS)) {
            return bonus.name.to_string();
        }
    }
    match mark {
        Some(FieldMark::Elite(k)) => {
            let kind = data::elite_kind(*k);
            format!("{base} {}", tr(kind.title)).trim().to_string()
        }
        _ => base.to_string(),
    }
}

/// A number as the meter reads it: `960`, `1.7k`, `2.3M`.
pub(super) fn short(n: f64) -> String {
    if n >= 1_000_000.0 {
        format!("{:.1}M", n / 1_000_000.0)
    } else if n >= 1_000.0 {
        format!("{:.1}k", n / 1_000.0)
    } else {
        format!("{}", n.round() as i64)
    }
}

/// The meter's rows: the spells by damage, the most first, each with its share of the whole.
pub(super) fn meter_rows(by: &[(u32, f64)]) -> Vec<(u32, f64, f32)> {
    let total: f64 = by.iter().map(|(_, d)| d).sum();
    let mut rows: Vec<(u32, f64, f32)> = by
        .iter()
        .filter(|(_, d)| *d > 0.0)
        .map(|&(s, d)| (s, d, if total > 0.0 { (d / total) as f32 } else { 0.0 }))
        .collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1));
    rows.truncate(METER_ROWS);
    rows
}

/// The frames follow the run: the hero's name, health, power and level; the target's while there
/// is one; the meter's rows when the damage moves on.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn refresh_frames(
    mut commands: Commands,
    run: Res<Run>,
    session: Res<Session>,
    selection: Res<Selection>,
    store: Res<super::profile::ProfileStore>,
    target: Res<HeroTarget>,
    font: Res<HudFont>,
    spells: Option<Res<crate::ui_action::Spells>>,
    units: Query<(
        &ObjectStore,
        Option<&Enemy>,
        Option<&crate::net::NetEntity>,
        Option<&FieldMark>,
    )>,
    mut texts: ParamSet<(
        Query<&mut Text, With<PlayerName>>,
        Query<&mut Text, With<PlayerLevel>>,
        Query<&mut Text, With<PlayerHealthText>>,
        Query<&mut Text, With<TargetName>>,
        Query<&mut Text, With<TargetLevel>>,
    )>,
    mut fills: ParamSet<(
        Query<&mut Node, With<PlayerHealth>>,
        Query<&mut Node, With<TargetHealth>>,
    )>,
    mut power: Query<&mut ImageNode, (With<PlayerPower>, With<hud::ArtDone>)>,
    mut portrait: Query<(Entity, Option<&UiArt>), (With<PlayerPortrait>, Without<PlayerPower>)>,
    mut shown: ParamSet<(
        Query<&mut Visibility, With<TargetFrame>>,
        Query<&mut Visibility, With<Meter>>,
    )>,
    rows: Query<Entity, With<MeterRows>>,
    mut last_meter: Local<Vec<(u32, u64)>>,
) {
    let set = |t: &mut Text, s: &str| {
        if t.0 != s {
            t.0 = s.to_string();
        }
    };
    // The hero.
    let name = if session.multiplayer() {
        session.name_of(session.slot).to_string()
    } else {
        store.profile.hero.name.clone()
    };
    for mut t in &mut texts.p0() {
        set(&mut t, &name);
    }
    for mut t in &mut texts.p1() {
        set(&mut t, &run.level.to_string());
    }
    let max = run.stats().max_hp.max(1.0);
    for mut t in &mut texts.p2() {
        set(
            &mut t,
            &format!("{} / {}", run.hp.max(0.0).round(), max.round()),
        );
    }
    let width = Val::Percent((run.hp / max * 100.0).clamp(0.0, 100.0));
    for mut n in &mut fills.p0() {
        if n.width != width {
            n.width = width;
        }
    }
    let want = power_color(run.class.power);
    for mut img in &mut power {
        if img.color != want {
            img.color = want;
        }
    }
    let sex = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .map_or(0, |l| l.2);
    let path = portrait_of(run.race, sex);
    for (e, art) in &mut portrait {
        if art.is_none_or(|a| a.path() != path) {
            commands
                .entity(e)
                .remove::<hud::ArtDone>()
                .insert(UiArt::crop(path, [0.0, 1.0, 0.0, 1.0]));
        }
    }

    // The target, while there is one.
    let seen = target.0.and_then(|e| units.get(e).ok());
    if let Ok(mut vis) = shown.p0().single_mut() {
        let want = if seen.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
    if let Some((store, enemy, net, mark)) = seen {
        let name = target_name(enemy, net.and_then(|n| n.display_id), mark, selection.stage);
        for mut t in &mut texts.p3() {
            set(&mut t, &name);
        }
        let level = super::units::level(store);
        for mut t in &mut texts.p4() {
            set(
                &mut t,
                &if level > 0 {
                    level.to_string()
                } else {
                    "??".into()
                },
            );
        }
        let pct = match enemy {
            Some(e) => (e.hp / e.max_hp.max(1.0)).clamp(0.0, 1.0) * 100.0,
            None => super::units::health_pct(store),
        };
        for mut n in &mut fills.p1() {
            let w = Val::Percent(pct);
            if n.width != w {
                n.width = w;
            }
        }
    }

    // The meter, rebuilt when its numbers move by a whole point.
    let key: Vec<(u32, u64)> = run.damage_by.iter().map(|&(s, d)| (s, d as u64)).collect();
    if let Ok(mut vis) = shown.p1().single_mut() {
        let want = if key.is_empty() {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *vis != want {
            *vis = want;
        }
    }
    if *last_meter == key {
        return;
    }
    *last_meter = key;
    let Ok(list) = rows.single() else { return };
    let f = &font.0;
    let top = meter_rows(&run.damage_by);
    let most = top.first().map_or(1.0, |r| r.1.max(1.0));
    commands.entity(list).despawn_related::<Children>();
    commands.entity(list).with_children(|l| {
        for (i, (spell, dmg, share)) in top.into_iter().enumerate() {
            let name = if spell == 0 {
                tr("Other").to_string()
            } else {
                hud::spell_name(spells.as_deref(), spell, "?")
            };
            l.spawn(Node {
                height: Val::Px(15.0),
                align_items: AlignItems::Center,
                column_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|r| {
                // The bar behind, as long as this spell's share of the top one's.
                r.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(17.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Percent((dmg / most * 88.0) as f32),
                        ..default()
                    },
                    BackgroundColor(if i == 0 {
                        Color::srgba(0.75, 0.2, 0.15, 0.75)
                    } else {
                        Color::srgba(0.2, 0.35, 0.7, 0.65)
                    }),
                ));
                r.spawn((
                    Node {
                        width: Val::Px(14.0),
                        height: Val::Px(14.0),
                        ..default()
                    },
                    ImageNode {
                        color: Color::srgba(0.0, 0.0, 0.0, 0.6),
                        ..default()
                    },
                    SpellIcon(spell),
                ));
                r.spawn(label(f, &format!("{}. {name}", i + 1), 10.0, Color::WHITE));
                r.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                r.spawn(label(
                    f,
                    &trf(
                        "{} ({}%)",
                        &[&short(dmg), &((share * 100.0).round() as u32).to_string()],
                    ),
                    10.0,
                    Color::WHITE,
                ));
            });
        }
    });
}

/// The line under the clock: when the stage's next boss comes, or where its final boss stands.
#[derive(Component)]
pub(super) struct NextBossText;

fn clock(secs: f32) -> String {
    let s = secs.ceil().max(0.0) as u32;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// What the line under the clock says: the next boss's time while the clock still has one to
/// bring, then the boss portal's, then that it is open; that the stage is cleared once the way
/// onward is there; nothing during the final boss's fight or the swarm.
pub(super) fn next_boss_line(run: &Run, rules: data::Rules) -> String {
    use super::legs::Gate;
    if run.swarm.is_some() {
        return String::new();
    }
    if run.gates.iter().any(|g| g.kind == Gate::Onward) {
        return tr("Stage cleared").to_string();
    }
    if run.gates.iter().any(|g| g.kind == Gate::Boss && g.open) {
        return tr("Boss portal open").to_string();
    }
    let last = data::BOSS_TIMES.len() - 1;
    let next = run.bosses as usize;
    if next < last {
        let left = rules.boss_time(next) - run.elapsed;
        trf("Next boss in {}", &[&clock(left)])
    } else if next == last {
        let left = rules.boss_time(last) - run.elapsed;
        trf("Boss portal in {}", &[&clock(left)])
    } else {
        String::new()
    }
}

/// The horde's wave beside it: the time to the next surge, or that one is on.
pub(super) fn wave_line(run: &Run) -> String {
    if run.swarm.is_some() || !run.begun {
        return String::new();
    }
    if data::surging(run.elapsed) {
        tr("Wave!").to_string()
    } else {
        trf("Wave in {}", &[&clock(data::next_surge_in(run.elapsed))])
    }
}

pub(super) fn refresh_next_boss(
    run: Res<Run>,
    session: Res<Session>,
    mut text: Query<&mut Text, With<NextBossText>>,
) {
    let boss = next_boss_line(&run, run.threat(session.rules));
    let wave = wave_line(&run);
    let want = match (boss.is_empty(), wave.is_empty()) {
        (false, false) => format!("{boss}  ·  {wave}"),
        (true, _) => wave,
        (_, true) => boss,
    };
    for mut t in &mut text {
        if t.0 != want {
            t.0 = want.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wave_line_counts_down_to_the_surge() {
        let mut run = Run::default();
        assert_eq!(wave_line(&run), "", "nothing before the run");
        run.begun = true;
        run.elapsed = 0.0;
        assert_eq!(
            wave_line(&run),
            format!("Wave in {}", clock(data::WAVE_EVERY - data::SURGE_SECS))
        );
        run.elapsed = data::WAVE_EVERY - 1.0;
        assert_eq!(wave_line(&run), "Wave!");
        run.swarm = Some(5.0);
        assert_eq!(wave_line(&run), "");
    }

    #[test]
    fn the_line_under_the_clock_follows_the_stage() {
        use super::super::legs::{Gate, GateView};
        let rules = data::Rules::SOLO;
        let mut run = Run::default();
        let r = run.threat(rules);
        run.elapsed = r.boss_time(0) - 65.0;
        assert_eq!(next_boss_line(&run, r), "Next boss in 01:05");
        run.bosses = (data::BOSS_TIMES.len() - 1) as u32;
        run.elapsed = r.boss_time(run.bosses as usize) - 3.2;
        assert_eq!(next_boss_line(&run, r), "Boss portal in 00:04");
        let gate = |kind, open| GateView {
            kind,
            pos: Vec3::ZERO,
            charge: 0.0,
            open,
        };
        run.gates = vec![gate(Gate::Boss, true)];
        assert_eq!(next_boss_line(&run, r), "Boss portal open");
        run.gates = vec![gate(Gate::Onward, true)];
        assert_eq!(next_boss_line(&run, r), "Stage cleared");
        run.gates.clear();
        run.bosses += 1;
        assert_eq!(next_boss_line(&run, r), "");
        run.swarm = Some(10.0);
        assert_eq!(next_boss_line(&run, r), "");
    }

    #[test]
    fn the_meter_lists_the_most_damage_first_with_its_share() {
        let rows = meter_rows(&[(1, 100.0), (2, 300.0), (3, 0.0), (4, 600.0)]);
        assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), [4, 2, 1]);
        assert!((rows[0].2 - 0.6).abs() < 1e-6);
        let many: Vec<(u32, f64)> = (0..20).map(|i| (i, f64::from(i) + 1.0)).collect();
        assert_eq!(meter_rows(&many).len(), METER_ROWS);
    }

    #[test]
    fn numbers_read_short() {
        assert_eq!(short(960.0), "960");
        assert_eq!(short(1_700.0), "1.7k");
        assert_eq!(short(2_340_000.0), "2.3M");
    }

    #[test]
    fn every_race_and_sex_has_a_portrait() {
        for race in 1..=8 {
            for sex in 0..=1 {
                assert!(portrait_of(race, sex).starts_with("Interface\\CharacterFrame\\"));
            }
        }
        assert_ne!(portrait_of(1, 0), portrait_of(1, 1));
    }

    #[test]
    fn a_guests_target_is_named_by_its_look() {
        let roster = data::roster(0);
        let e = &roster.enemies[0];
        assert_eq!(target_name(None, Some(e.display), None, 0), e.name);
        let elite = target_name(None, Some(e.display), Some(&FieldMark::Elite(0)), 0);
        assert!(elite.ends_with(tr(data::ELITES[0].title)), "{elite}");
        assert_eq!(target_name(None, None, None, 0), "");
    }
}
