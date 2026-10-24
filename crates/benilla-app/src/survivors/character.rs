//! The main screen: the title, the tabs, and the Character tab's three columns — the class on
//! the left, the hero in the middle, the zone on the right — over the slowly orbiting hero.
//!
//! It is built from [`super::kit`] and drives the same [`Selection`] and profile as the glue
//! menu ([`super::menu`]), which it replaces; `WOW_NEW_MENU=0` (`?new_menu=0` on the web) brings
//! the glue menu back.
//!
//! Today: the class column (the class grid, the class's description, its starting health with
//! everything the profile brings in, its starting spell, the zones open, and its spell pool with
//! tooltips) and the hero column (faction, name with a race-flavoured Random, race and sex,
//! an open stage where the live hero turns, the equipment slots and the race), the zone column
//! (every zone, the picked one opened with its horde, its bosses and when they come, and the
//! record of past runs there) and the bottom bar (run options, the way in, co-op).

use bevy::prelude::*;

use super::data::{self, ClassDef};
use super::hud::{self, TipLine};
use super::kit::{self, theme, FieldDone, IconSource, KitClicks, KitFonts, TextField, Tooltip};
use super::profile::ProfileStore;
use super::{menu, names, Owned, Phase, Rng, Run, Selection, Vitals};

/// The new screen replaces the glue menu; `WOW_NEW_MENU=0` (`?new_menu=0`) brings the glue menu
/// back.
pub(super) fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| crate::webenv::var("WOW_NEW_MENU").is_none_or(|v| v != "0"))
}

pub(super) fn plugin(app: &mut App) {
    if !enabled() {
        return;
    }
    app.add_systems(
        Update,
        (
            build_screen,
            show_screen,
            clicks,
            name_edits,
            refresh_class_column,
            refresh_hero_card,
            refresh_zone_list,
            refresh_bottom_bar,
            adopt_name,
        )
            .chain()
            .after(hud::show_panels),
    );
}

/// The tabs, in the screenshot's order; only Character is built so far.
const TABS: [&str; 7] = [
    "Character",
    "Collection",
    "Drop Tables",
    "Codex",
    "Talents",
    "Achievements",
    "Rewards",
];

#[derive(Component)]
struct ScreenRoot;

/// The class column's body, rebuilt when the pick changes.
#[derive(Component)]
struct ClassColumn;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
enum ScreenAction {
    Tab(usize),
    Class(usize),
    Race(u8),
    Sex(u8),
    RandomName,
    Zone(usize),
    Enter,
    Coop,
}

/// The zone list's rows, rebuilt when the zone picked changes.
#[derive(Component)]
struct ZoneList;

/// The bottom bar, rebuilt when the zone or the lobby changes.
#[derive(Component)]
struct BottomBar;

/// The hero's name field.
#[derive(Component)]
struct NameField;

/// The hero card's parts that follow the pick: the faction badge, the race-and-class line, the
/// race and sex pickers, and the race section under the stage.
#[derive(Component)]
struct HeroFaction;
#[derive(Component)]
struct HeroLine;
#[derive(Component)]
struct HeroPickers;
#[derive(Component)]
struct RaceSection;

fn build_screen(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    built: Query<(), With<ScreenRoot>>,
) {
    let Some(fonts) = fonts else { return };
    if !built.is_empty() {
        return;
    }
    let f = &*fonts;
    commands
        .spawn((
            ScreenRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(24.0), Val::Px(14.0)),
                row_gap: Val::Px(10.0),
                display: Display::None,
                ..default()
            },
            GlobalZIndex(410),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn(kit::text(&f.title, "Azeroth Survivors", 44.0, theme::GOLD));
            let tabs: Vec<(&str, ScreenAction)> = TABS
                .iter()
                .enumerate()
                .map(|(i, t)| (*t, ScreenAction::Tab(i)))
                .collect();
            let ids = kit::tab_bar(root, f, &tabs, 0);
            for (i, id) in ids.into_iter().enumerate().skip(1) {
                root.commands().entity(id).insert((
                    kit::Disabled(true),
                    Tooltip::simple(TABS[i], "Coming soon."),
                ));
            }
            // The three columns; the middle one stays open so the hero shows through.
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                column_gap: Val::Px(14.0),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            })
            .with_children(|cols| {
                kit::panel(
                    cols,
                    f,
                    Some("Class"),
                    Node {
                        width: Val::Px(300.0),
                        ..default()
                    },
                    |p| {
                        p.spawn((
                            ClassColumn,
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(8.0),
                                ..default()
                            },
                        ));
                    },
                );
                hero_column(cols, f);
                kit::panel(
                    cols,
                    f,
                    Some("Zone"),
                    Node {
                        width: Val::Px(320.0),
                        min_height: Val::Px(0.0),
                        ..default()
                    },
                    |p| {
                        kit::scroll_area(
                            p,
                            Node {
                                flex_grow: 1.0,
                                flex_basis: Val::Px(0.0),
                                row_gap: Val::Px(5.0),
                                ..default()
                            },
                            |list| {
                                list.spawn((
                                    ZoneList,
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(5.0),
                                        ..default()
                                    },
                                ));
                            },
                        );
                    },
                );
            });
            // The bar under the columns: run options, the way in, co-op.
            root.spawn((
                BottomBar,
                Node {
                    column_gap: Val::Px(14.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            root.spawn(kit::text(
                &f.body,
                "WASD to run · the mouse to look · 1 2 3 picks a card · Enter to play",
                12.0,
                theme::DIM,
            ));
        });
}

/// Shown at the menu, in place of the glue screen.
fn show_screen(
    phase: Res<State<Phase>>,
    mut roots: Query<(&mut Visibility, &mut Node), With<ScreenRoot>>,
) {
    let on = *phase.get() == Phase::Menu;
    for (mut v, mut n) in &mut roots {
        let want = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
        let display = if on { Display::Flex } else { Display::None };
        if n.display != display {
            n.display = display;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn clicks(
    phase: Res<State<Phase>>,
    clicks: Res<KitClicks>,
    buttons: Query<(Entity, &ScreenAction)>,
    creatures: Option<Res<crate::entities::Creatures>>,
    session: Res<super::Session>,
    mut selection: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut store: ResMut<ProfileStore>,
    mut next: ResMut<NextState<Phase>>,
    mut rng: ResMut<Rng>,
    mut name_field: Query<&mut TextField, With<NameField>>,
) {
    if *phase.get() != Phase::Menu {
        return;
    }
    let Some(&action) = clicks.first(&buttons) else {
        return;
    };
    match action {
        ScreenAction::Tab(_) => {}
        ScreenAction::Class(_) | ScreenAction::Race(_) | ScreenAction::Sex(_) => {
            let Some(creatures) = creatures.as_deref() else {
                return;
            };
            let all = menu::all_looks(creatures);
            match action {
                ScreenAction::Class(c) => menu::pick_class(&mut selection, &all, c),
                ScreenAction::Race(r) => menu::pick_race(&mut selection, &all, r),
                ScreenAction::Sex(s) => menu::pick_sex(&mut selection, &all, s),
                _ => {}
            }
            menu::save_pick(&selection, &mut store);
        }
        ScreenAction::RandomName => {
            let race = menu::current_look(&selection).map_or(1, |l| l.0);
            let name = names::random_name(race, || rng.f32());
            store.edit(|p| p.hero.name.clone_from(&name));
            for mut field in &mut name_field {
                field.value.clone_from(&name);
            }
        }
        ScreenAction::Zone(i) => {
            if selection.map != i && store.profile.zone_open(i) {
                selection.map = i;
                selection.version += 1;
                menu::save_pick(&selection, &mut store);
            }
        }
        ScreenAction::Coop => next.set(Phase::Lobby),
        ScreenAction::Enter => {
            if session.in_lobby {
                next.set(Phase::Lobby);
            } else if store.profile.zone_open(selection.map)
                && hud::start_run(&mut run, &mut selection)
            {
                next.set(Phase::Playing);
            }
        }
    }
}

/// How many of the campaign's zones the profile has opened.
fn open_zones(store: &ProfileStore) -> usize {
    (0..data::MAPS.len())
        .filter(|&i| store.profile.zone_open(i))
        .count()
}

/// What the class column shows, to rebuild it only when that changes.
#[derive(PartialEq, Clone, Debug)]
struct ClassView {
    class: Option<usize>,
    race: u8,
    spells_ready: bool,
    bonuses: usize,
    open: usize,
}

/// The class column, rebuilt when the pick, the profile or the spell names change.
#[allow(clippy::too_many_arguments)]
fn refresh_class_column(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    store: Res<ProfileStore>,
    spells: Option<Res<crate::ui_action::Spells>>,
    column: Query<Entity, With<ClassColumn>>,
    mut shown: Local<Option<ClassView>>,
) {
    let (Some(fonts), Ok(column)) = (fonts, column.single()) else {
        return;
    };
    let race = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .map_or(0, |l| l.1);
    let (meta, procs) = store.profile.run_bonuses(race);
    let view = ClassView {
        class: selection.class,
        race,
        spells_ready: spells.is_some(),
        bonuses: meta.len() + procs.len(),
        open: open_zones(&store),
    };
    if shown.as_ref() == Some(&view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let spells = spells.as_deref();
    commands.entity(column).despawn_related::<Children>();
    commands.entity(column).with_children(|p| {
        kit::grid(p, 3, 10.0, |g| {
            for (i, c) in data::CLASSES.iter().enumerate() {
                kit::icon_button(
                    g,
                    f,
                    class_icon(c),
                    42.0,
                    Some(c.name),
                    selection.class == Some(i),
                    ScreenAction::Class(i),
                );
            }
        });
        let Some(class) = selection.class_def() else {
            p.spawn(kit::text(&f.body, "Pick a class.", 13.0, theme::DIM));
            return;
        };
        let hero = Vitals::new(class, race).with_meta(meta, &procs);
        let stats = hero.stats();
        p.spawn(kit::text(&f.body, class.name, 20.0, hud::rgb(class.color)));
        p.spawn(kit::text(&f.body, class.blurb, 13.0, theme::TEXT));
        p.spawn(Node {
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            kit::stat_tile(r, f, "Health", &format!("{:.0}", stats.max_hp));
            let start = data::ability(class.start);
            kit::stat_tile(
                r,
                f,
                "Starts with",
                &hud::spell_name(spells, start.spell, start.key),
            );
            kit::stat_tile(
                r,
                f,
                "Zones open",
                &format!("{} of {}", open_zones(&store), data::MAPS.len()),
            );
        });
        p.spawn(kit::text(&f.body, "SPELL POOL", 10.0, theme::DIM));
        p.spawn(Node {
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|r| {
            for key in class.pool {
                let def = data::ability(key);
                let lines = spell_tip(def, &stats, spells);
                kit::icon(r, IconSource::Spell(def.spell), 26.0, Some(Tooltip(lines)));
            }
        });
    });
}

/// The class's icon from the character-create sheet, `CLASS_ICON_TCOORDS`.
fn class_icon(class: &ClassDef) -> IconSource {
    match crate::glue::art::class_tc(class.id) {
        Some(tc) => IconSource::Atlas(
            "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Classes".into(),
            tc,
        ),
        None => IconSource::Spell(data::ability(class.start).spell),
    }
}

/// A pool spell's tooltip: what it does at its first rank, with the class's starting numbers.
fn spell_tip(
    def: &'static data::AbilityDef,
    stats: &super::Stats,
    spells: Option<&crate::ui_action::Spells>,
) -> Vec<TipLine> {
    let owned = Owned {
        def,
        level: 1,
        cd: 0.0,
    };
    hud::ability_tip(&owned, stats, spells)
}

/// The ten slots the hero wears, with the paper doll's empty-slot art.
const EQUIPMENT: [(&str, &str); 10] = [
    ("Head", "Head"),
    ("Shoulders", "Shoulder"),
    ("Chest", "Chest"),
    ("Wrists", "Wrists"),
    ("Hands", "Hands"),
    ("Waist", "Waist"),
    ("Legs", "Legs"),
    ("Feet", "Feet"),
    ("Main Hand", "MainHand"),
    ("Off Hand", "SecondaryHand"),
];

/// The Alliance's races; the rest are the Horde's.
fn alliance(race: u8) -> bool {
    matches!(race, 1 | 3 | 4 | 7)
}

/// The races in the character-create order.
const RACES: [u8; 8] = [1, 3, 7, 4, 2, 5, 6, 8];

/// The middle column: the hero card on top, the open stage where the live hero turns, and the
/// equipment and race under it.
fn hero_column(cols: &mut ChildSpawnerCommands, f: &KitFonts) {
    cols.spawn(Node {
        flex_grow: 1.0,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: Val::Px(8.0),
        max_width: Val::Px(560.0),
        ..default()
    })
    .with_children(|c| {
        kit::panel(
            c,
            f,
            None,
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            |p| {
                p.spawn(Node {
                    column_gap: Val::Px(8.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|r| {
                    r.spawn((HeroFaction, Node::default()));
                    kit::text_field(
                        r,
                        f,
                        "",
                        230.0,
                        names::MAX_NAME,
                        names::name_char,
                        NameField,
                    );
                    kit::button(
                        r,
                        f,
                        "Random",
                        None,
                        kit::ButtonLook::PLAIN,
                        ScreenAction::RandomName,
                    );
                });
                p.spawn((
                    HeroLine,
                    Node {
                        flex_direction: FlexDirection::Column,
                        ..default()
                    },
                ));
                p.spawn((
                    HeroPickers,
                    Node {
                        column_gap: Val::Px(4.0),
                        align_items: AlignItems::Center,
                        flex_wrap: FlexWrap::Wrap,
                        ..default()
                    },
                ));
            },
        );
        // The stage: no fill, so the hero orbiting in the world shows through its frame.
        c.spawn((
            Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                min_height: Val::Px(140.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BorderColor::all(theme::BRASS.with_alpha(0.55)),
            Pickable::IGNORE,
        ));
        kit::panel(
            c,
            f,
            Some("Equipment"),
            Node {
                width: Val::Percent(100.0),
                ..default()
            },
            |p| {
                p.spawn(Node {
                    column_gap: Val::Px(4.0),
                    justify_content: JustifyContent::Center,
                    flex_wrap: FlexWrap::Wrap,
                    ..default()
                })
                .with_children(|r| {
                    for (label, art) in EQUIPMENT {
                        kit::icon(
                            r,
                            IconSource::Texture(format!(
                                "Interface\\PaperDoll\\UI-PaperDoll-Slot-{art}"
                            )),
                            30.0,
                            Some(Tooltip(vec![
                                TipLine::new(label, Color::WHITE).header(),
                                TipLine::new("Empty", theme::DIM),
                            ])),
                        );
                    }
                });
                p.spawn(Node {
                    column_gap: Val::Px(8.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|r| {
                    for (title, sub) in [
                        ("Armory", "Race, look, gear and drop tables"),
                        ("Talents", "Spend your talent points"),
                    ] {
                        let b = kit::button(r, f, title, Some(sub), kit::ButtonLook::ACTION, ());
                        r.commands()
                            .entity(b)
                            .insert((kit::Disabled(true), Tooltip::simple(title, "Coming soon.")));
                    }
                });
                p.spawn((
                    RaceSection,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(3.0),
                        ..default()
                    },
                ));
            },
        );
    });
}

/// The name field's edits go on the profile, tidied; Random rolls one in the race's sound.
fn name_edits(
    mut done: MessageReader<FieldDone>,
    mut fields: Query<&mut TextField, With<NameField>>,
    mut store: ResMut<ProfileStore>,
) {
    for d in done.read() {
        let Ok(mut field) = fields.get_mut(d.field) else {
            continue;
        };
        let name = names::clean_name(&d.value);
        if name.chars().count() < 2 {
            // Too short to be a name: the last one stands.
            field.value.clone_from(&store.profile.hero.name);
            continue;
        }
        if field.value != name {
            field.value.clone_from(&name);
        }
        store.edit(|p| p.hero.name = name);
    }
}

/// What the hero card shows, to rebuild it only when that changes.
#[derive(PartialEq, Clone, Debug)]
struct HeroView {
    class: Option<usize>,
    look: Option<(u8, u8)>,
    looks_known: bool,
}

/// The parts of the hero card that follow the pick; the name too, when the profile has none yet.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_hero_card(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    creatures: Option<Res<crate::entities::Creatures>>,
    mut store: ResMut<ProfileStore>,
    mut rng: ResMut<Rng>,
    mut name: Query<&mut TextField, With<NameField>>,
    mut parts: ParamSet<(
        Query<Entity, With<HeroFaction>>,
        Query<Entity, With<HeroLine>>,
        Query<Entity, With<HeroPickers>>,
        Query<Entity, With<RaceSection>>,
    )>,
    mut shown: Local<Option<HeroView>>,
) {
    let Some(fonts) = fonts else { return };
    let look = menu::current_look(&selection);
    // A hero always has a name: the profile's, or one rolled for the race.
    if let Ok(mut field) = name.single_mut() {
        if store.profile.hero.name.is_empty() {
            if let Some((race, _)) = look {
                let rolled = names::random_name(race, || rng.f32());
                store.edit(|p| p.hero.name.clone_from(&rolled));
            }
        }
        if field.value.is_empty() && !store.profile.hero.name.is_empty() {
            field.value.clone_from(&store.profile.hero.name);
        }
    }
    let view = HeroView {
        class: selection.class,
        look,
        looks_known: creatures.is_some(),
    };
    if shown.as_ref() == Some(&view) {
        return;
    }
    let (Ok(faction), Ok(line), Ok(pickers), Ok(race_section)) = (
        parts.p0().single(),
        parts.p1().single(),
        parts.p2().single(),
        parts.p3().single(),
    ) else {
        return;
    };
    *shown = Some(view);
    let f = &*fonts;
    let all = creatures
        .as_deref()
        .map(menu::all_looks)
        .unwrap_or_default();
    for e in [faction, line, pickers, race_section] {
        commands.entity(e).despawn_related::<Children>();
    }
    let Some((race, sex)) = look else {
        return;
    };
    let class = selection.class_def();

    commands.entity(faction).with_children(|b| {
        let (label, color) = if alliance(race) {
            ("ALLIANCE", Color::srgb(0.25, 0.5, 1.0))
        } else {
            ("HORDE", Color::srgb(0.9, 0.2, 0.15))
        };
        b.spawn((
            Node {
                padding: UiRect::axes(Val::Px(7.0), Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(color.with_alpha(0.2)),
            BorderColor::all(color),
        ))
        .with_children(|c| {
            c.spawn(kit::text(&f.body, label, 10.0, color));
        });
    });

    commands.entity(line).with_children(|l| {
        l.spawn(Node {
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn(kit::text(
                &f.body,
                crate::char_select::race_name(race),
                17.0,
                theme::TEXT,
            ));
            if let Some(class) = class {
                r.spawn(kit::text(&f.body, class.name, 17.0, hud::rgb(class.color)));
            }
        });
        l.spawn(kit::text(
            &f.body,
            if sex == 1 { "Female" } else { "Male" },
            13.0,
            theme::DIM,
        ));
    });

    commands.entity(pickers).with_children(|r| {
        for race_id in RACES {
            let playable = all.is_empty() || menu::race_playable(&all, race_id);
            let icon = match crate::glue::art::race_tc(race_id, sex) {
                Some(tc) => IconSource::Atlas(
                    "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Races".into(),
                    tc,
                ),
                None => continue,
            };
            let e = kit::icon_button(
                r,
                f,
                icon,
                26.0,
                None,
                race_id == race,
                ScreenAction::Race(race_id),
            );
            if !playable {
                r.commands().entity(e).insert(Visibility::Hidden);
            }
            let name = crate::char_select::race_name(race_id);
            // The client's looks decide; without them, vanilla's own pairs.
            let keeps_class = if all.is_empty() {
                class.is_none_or(|c| data::race_classes(race_id).contains(&c.id))
            } else {
                selection
                    .class
                    .and_then(|c| all.get(c))
                    .is_some_and(|looks| looks.iter().any(|l| l.1 == race_id))
            };
            let note = if keeps_class {
                format!("Play as a {name}.")
            } else {
                format!("Picking {name} changes your class.")
            };
            r.commands().entity(e).insert(Tooltip::simple(name, note));
        }
        r.spawn(Node {
            width: Val::Px(10.0),
            ..default()
        });
        for (s, label) in [(0u8, "Male"), (1, "Female")] {
            let b = kit::button(
                r,
                f,
                label,
                None,
                kit::ButtonLook::PLAIN,
                ScreenAction::Sex(s),
            );
            r.commands().entity(b).insert(kit::Selected(s == sex));
        }
    });

    commands.entity(race_section).with_children(|r| {
        r.spawn(kit::text(
            &f.title,
            crate::char_select::race_name(race),
            18.0,
            theme::GOLD,
        ));
        r.spawn(kit::text(
            &f.body,
            if alliance(race) {
                "Fights for the Alliance."
            } else {
                "Fights for the Horde."
            },
            12.0,
            theme::DIM,
        ));
        r.spawn(kit::text(&f.body, "Racials", 13.0, theme::GOLD));
        for def in data::racials(race) {
            r.spawn(Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                kit::icon(
                    row,
                    IconSource::Spell(def.spell),
                    24.0,
                    Some(racial_tip(def)),
                );
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.body, def.name, 12.0, theme::TEXT));
                    c.spawn(kit::text(&f.body, def.text, 11.0, theme::GREEN));
                });
            });
        }
    });
}

/// A racial's tooltip: its name, what it does, and whether it is always on or fires.
fn racial_tip(def: &data::RacialDef) -> Tooltip {
    let kind = if def.proc.is_some() {
        "Fires on its own"
    } else {
        "Always on"
    };
    Tooltip(vec![
        TipLine::new(def.name, Color::WHITE)
            .header()
            .right("Racial", theme::DIM),
        TipLine::new(kind, theme::DIM),
        TipLine::new(def.text, theme::GREEN),
    ])
}

/// Minutes and seconds, "2:15".
fn mmss(secs: f32) -> String {
    let s = secs.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A zone's bosses and when each comes, under the solo rules: `(time, name)`, the final boss last.
fn boss_timeline(zone: usize) -> Vec<(f32, &'static str)> {
    let rules = data::Rules::SOLO;
    let roster = data::roster(zone);
    (0..roster.bosses.len())
        .map(|k| (rules.boss_time(k), roster.bosses[k].name))
        .collect()
}

/// A zone's horde by name, weakest first, each once.
fn enemy_names(zone: usize) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for e in &data::roster(zone).enemies {
        if !out.contains(&e.name) {
            out.push(e.name);
        }
    }
    out
}

/// The zone rows: the picked one opened (its title, the horde, the bosses and when they come,
/// what winning means), the other open ones as a name to pick, the locked ones with what opens
/// them.
fn refresh_zone_list(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    store: Res<ProfileStore>,
    list: Query<Entity, With<ZoneList>>,
    mut shown: Local<Option<(usize, usize, usize)>>,
) {
    let (Some(fonts), Ok(list)) = (fonts, list.single()) else {
        return;
    };
    let view = (selection.map, store.profile.zones.len(), open_zones(&store));
    if *shown == Some(view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    commands.entity(list).despawn_related::<Children>();
    commands.entity(list).with_children(|l| {
        for (i, map) in data::MAPS.iter().enumerate() {
            let open = store.profile.zone_open(i);
            let picked = open && i == selection.map;
            let record = store.profile.zones.get(map.name);
            let button = l
                .spawn((
                    Button,
                    kit::ButtonLook::PLAIN,
                    kit::Selected(picked),
                    kit::Disabled(!open),
                    ScreenAction::Zone(i),
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                        row_gap: Val::Px(3.0),
                        border: UiRect::all(Val::Px(if picked { 2.0 } else { 1.0 })),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(theme::BUTTON),
                    BorderColor::all(theme::BRASS),
                    BoxShadow::new(
                        Color::NONE,
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(0.0),
                        Val::Px(8.0),
                    ),
                ))
                .id();
            l.commands().entity(button).with_children(|b| {
                b.spawn(kit::text(
                    &f.title,
                    map.name,
                    if picked { 22.0 } else { 17.0 },
                    if picked {
                        theme::GOLD
                    } else if open {
                        theme::TEXT
                    } else {
                        theme::DIM
                    },
                ));
                if !open {
                    let hint = data::unlock_hint(i).unwrap_or_default();
                    b.spawn(kit::text(&f.body, format!("Locked · {hint}"), 11.0, theme::RED));
                    return;
                }
                let mut sub = map.zone.to_string();
                if let Some(r) = record {
                    if r.wins > 0 {
                        sub.push_str(&format!(" · won {}×", r.wins));
                    } else if r.runs > 0 {
                        sub.push_str(&format!(" · best {}", mmss(r.best_time)));
                    }
                }
                b.spawn(kit::text(&f.body, sub, 11.0, theme::DIM));
                if !picked {
                    return;
                }
                b.spawn(kit::text(&f.body, map.victory, 12.0, theme::TEXT));
                let names = enemy_names(i);
                let shown_names: Vec<&str> = names.iter().take(10).copied().collect();
                let more = names.len().saturating_sub(shown_names.len());
                let mut line = shown_names.join(", ");
                if more > 0 {
                    line.push_str(&format!(" and {more} more"));
                }
                b.spawn(kit::text(&f.body, line, 11.0, theme::DIM));
                b.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(5.0),
                    row_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|chips| {
                    let timeline = boss_timeline(i);
                    let last = timeline.len().saturating_sub(1);
                    for (k, (t, name)) in timeline.iter().enumerate() {
                        kit::chip(chips, f, &format!("{} · {name}", mmss(*t)), k == last);
                    }
                });
                if let Some((_, final_boss)) = boss_timeline(i).last() {
                    b.spawn(kit::text(
                        &f.body,
                        format!("Defeat {final_boss} to win."),
                        11.0,
                        theme::GOLD,
                    ));
                }
            });
        }
    });
}

/// Run Options, the way into the zone picked, and co-op.
fn refresh_bottom_bar(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    session: Res<super::Session>,
    store: Res<ProfileStore>,
    bar: Query<Entity, With<BottomBar>>,
    mut shown: Local<Option<(usize, bool, bool)>>,
) {
    let (Some(fonts), Ok(bar)) = (fonts, bar.single()) else {
        return;
    };
    let open = store.profile.zone_open(selection.map);
    let view = (selection.map, session.in_lobby, open);
    if *shown == Some(view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let map = selection.map_def();
    let timeline = boss_timeline(selection.map);
    let final_boss = timeline.last().map_or("", |t| t.1);
    commands.entity(bar).despawn_related::<Children>();
    commands.entity(bar).with_children(|r| {
        let options = kit::button(
            r,
            f,
            "Run Options",
            Some("Normal difficulty"),
            kit::ButtonLook::PLAIN,
            (),
        );
        r.commands().entity(options).insert((
            kit::Disabled(true),
            Tooltip::simple("Run Options", "Difficulty and run modifiers arrive soon."),
        ));
        let sub = if open || session.in_lobby {
            format!(
                "Defeat {final_boss} at {} · {} bosses on the way",
                mmss(data::Rules::SOLO.duration),
                timeline.len()
            )
        } else {
            data::unlock_hint(selection.map).unwrap_or_default()
        };
        let enter = kit::big_button(
            r,
            f,
            &format!("Enter {}", map.name),
            &sub,
            ScreenAction::Enter,
        );
        if !open && !session.in_lobby {
            r.commands().entity(enter).insert(kit::Disabled(true));
        }
        let (title, sub) = if session.in_lobby {
            ("Co-op", "Back to your lobby")
        } else {
            ("Co-op", "Play with friends")
        };
        kit::button(
            r,
            f,
            title,
            Some(sub),
            kit::ButtonLook::PLAIN,
            ScreenAction::Coop,
        );
    });
}

/// The hero's name is the player's name in a lobby, unless `WOW_NAME` says otherwise.
fn adopt_name(store: Res<ProfileStore>, mut net: ResMut<super::net::Net>) {
    if !store.is_changed() || crate::webenv::var("WOW_NAME").is_some() || net.lobby.is_some() {
        return;
    }
    let name = &store.profile.hero.name;
    if !name.is_empty() && net.name != *name {
        net.name.clone_from(name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_has_its_icon_from_the_sheet() {
        for c in data::CLASSES {
            assert!(
                matches!(class_icon(c), IconSource::Atlas(..)),
                "{} has no cell on the class sheet",
                c.name
            );
        }
    }

    #[test]
    fn every_pool_spell_has_a_tooltip() {
        for c in data::CLASSES {
            let stats = Vitals::new(c, 1).stats();
            for key in c.pool {
                let lines = spell_tip(data::ability(key), &stats, None);
                assert!(!lines.is_empty(), "{} / {key}", c.name);
            }
        }
    }

    #[test]
    fn the_health_shown_counts_the_class_head_start() {
        let warrior = data::CLASSES.iter().find(|c| c.id == 1).unwrap();
        let mage = data::CLASSES.iter().find(|c| c.id == 8).unwrap();
        assert!(Vitals::new(warrior, 1).stats().max_hp > Vitals::new(mage, 1).stats().max_hp);
    }

    #[test]
    fn races_split_into_the_two_factions() {
        let alliance_count = RACES.iter().filter(|r| alliance(**r)).count();
        assert_eq!(alliance_count, 4);
        assert!(alliance(1) && !alliance(8));
        // Every race has its face on the create sheet, both sexes.
        for r in RACES {
            assert!(
                crate::glue::art::race_tc(r, 0).is_some()
                    && crate::glue::art::race_tc(r, 1).is_some()
            );
        }
    }

    #[test]
    fn every_zone_shows_its_bosses_in_order_ending_on_the_final_one() {
        for i in 0..data::MAPS.len() {
            let t = boss_timeline(i);
            assert_eq!(t.len(), data::BOSS_TIMES.len(), "{}", data::MAPS[i].name);
            assert!(t.windows(2).all(|w| w[0].0 < w[1].0));
            assert_eq!(mmss(t.last().unwrap().0), "16:00");
            assert!(!enemy_names(i).is_empty());
        }
        let elwynn: Vec<String> = boss_timeline(0)
            .iter()
            .map(|(t, n)| format!("{} {n}", mmss(*t)))
            .collect();
        assert_eq!(
            elwynn,
            [
                "3:00 Hogger",
                "6:00 Mother Fang",
                "9:00 Rhahk'Zor",
                "12:00 Mr. Smite",
                "16:00 Edwin VanCleef",
            ]
        );
    }

    #[test]
    fn clocks_read_as_minutes() {
        assert_eq!(mmss(0.0), "0:00");
        assert_eq!(mmss(900.0), "15:00");
        assert_eq!(mmss(135.4), "2:15");
    }

    #[test]
    fn the_tabs_match_the_screen() {
        assert_eq!(TABS[0], "Character");
        assert_eq!(TABS.len(), 7);
    }
}
