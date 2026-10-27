//! The main screen: the title, the tabs, and the Character tab's three columns — the class on
//! the left, the hero in the middle, the zone on the right — over the slowly orbiting hero.
//!
//! It is built from [`super::kit`] and drives the same [`Selection`] and profile as the glue
//! menu ([`super::menu`]), which it replaces; `WOW_NEW_MENU=0` (`?new_menu=0` on the web) brings
//! the glue menu back.
//!
//! Today: the class column (the class grid, the class's description, its starting health with
//! everything the profile brings in, its starting spell, the zones open, and its spell pool with
//! tooltips), the hero column (the hero's portrait with its faction and class, its name with a
//! race-flavoured Random, race and sex, the equipment slots, the collection, the Armory and
//! Talents, and the racials), the zone column (every zone, the picked one opened with its horde,
//! its bosses and when they come, and the record of past runs there) and the bottom bar (run
//! options, the way in, co-op).
//!
//! The Armory, opened from the portrait, the Armory button or an equipment slot, takes the whole
//! screen: race and sex and the slots on the left, the hero on a stage a drag turns in the middle,
//! the appearance dials and the pieces owned for the slot picked on the right.
//!
//! The hero is the live one in the world: the camera frames it into whichever of the portrait or
//! the stage is on screen ([`super::hero::MenuStage`]), so neither may carry a fill.

use bevy::prelude::*;

use super::data::{self, ClassDef};
use super::hero::{StageDrag, StageFrame};
use super::hud::{self, TipLine};
use super::kit::{self, theme, FieldDone, IconSource, KitClicks, KitFonts, TextField, Tooltip};
use super::lang::{self, tr, trf};
use super::profile::ProfileStore;
use super::{menu, names, outfit, Owned, Phase, Rng, Run, Selection, Vitals};

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
    app.init_resource::<Page>()
        .init_resource::<ArmorySlot>()
        .add_systems(
            Update,
            (
                (
                    rebuild_on_language,
                    build_screen,
                    show_screen,
                    clicks,
                    name_edits,
                    refresh_class_column,
                    refresh_hero_card,
                    refresh_equipment,
                    refresh_hero_buttons,
                    show_pages,
                    refresh_collection,
                    refresh_talents,
                    refresh_drops,
                    refresh_codex,
                    refresh_achievements,
                )
                    .chain(),
                (
                    refresh_options,
                    refresh_zone_list,
                    refresh_bottom_bar,
                    adopt_name,
                )
                    .chain(),
            )
                .chain()
                // The camera, placed earlier in the frame, frames the hero where the last
                // frame's layout put the stage.
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

/// The pages the screen shows in its columns' place: the tabs built so far, and the Armory.
const CHARACTER: usize = 0;
const COLLECTION: usize = 1;
const DROPS: usize = 2;
const CODEX: usize = 3;
const TALENTS: usize = 4;
const ACHIEVEMENTS: usize = 5;
const REWARDS: usize = 6;
const ARMORY: usize = 100;
const OPTIONS: usize = 101;
const BUILT_TABS: [usize; 7] = [
    CHARACTER,
    COLLECTION,
    DROPS,
    CODEX,
    TALENTS,
    ACHIEVEMENTS,
    REWARDS,
];

/// The page shown.
#[derive(Resource, Default)]
struct Page(usize);

#[derive(Component)]
struct PageRoot(usize);

#[derive(Component)]
struct TabButton(usize);

/// The screen's parts that only some pages show: the title over the tabs, the Armory's own
/// header, and the bar under the columns with its hint.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Chrome {
    Main,
    Armory,
    Bar,
}

/// The slot the Armory picks pieces for, if one is picked.
#[derive(Resource, Default)]
struct ArmorySlot(Option<usize>);

/// The Collection tab's body, and the collection bar under the equipment.
#[derive(Component)]
struct CollectionBody;
#[derive(Component)]
struct CollectionBar;
/// The Armory and Talents buttons under the collection bar.
#[derive(Component)]
struct HeroButtons;
/// The Armory header's line: the class, the pieces collected.
#[derive(Component)]
struct ArmorySub;
/// The Armory's slot list, the pieces owned for the slot picked, and what the gear worn gives.
#[derive(Component)]
struct ArmorySlots;
#[derive(Component)]
struct ArmoryPicker;
#[derive(Component)]
struct ArmoryNote;
#[derive(Component)]
struct TalentsBody;
#[derive(Component)]
struct DropsBody;
#[derive(Component)]
struct CodexBody;
#[derive(Component)]
struct AchievementsBody;
#[derive(Component)]
struct OptionsBody;

/// One Options setting a button steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Setting {
    Master,
    Music,
    Sfx,
    Sens,
    Polish,
    Shake,
}

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
    /// Turn one appearance dial on by one, or back by one.
    Dial(usize, bool),
    /// Roll every appearance dial.
    RandomLook,
    /// Open the Armory on one of the ten slots ([`data::GearSlot::ALL`]).
    Equip(usize),
    /// Pick one of the ten slots in the Armory.
    Slot(usize),
    /// Turn the hero on the Armory's stage, a step left (−1) or right (1).
    Turn(i8),
    /// Open the Armory.
    Armory,
    /// Wear an item ([`data::ITEMS`] index) from the Armory.
    Wear(usize),
    /// Take off what one of the ten slots wears.
    Unwear(usize),
    /// Back to the Character tab.
    Back,
    /// Learn a rank: tree and talent, by index in the class's trees.
    Learn(usize, usize),
    /// Train a talent point with gold.
    TrainPoint,
    /// Take back every point the class has spent.
    ResetTalents,
    /// The next difficulty open in the zone picked.
    RunOptions,
    /// Claim one achievement's reward ([`data::ACHIEVEMENTS`] index), or all of them.
    Claim(usize),
    ClaimAll,
    /// Open the Options.
    Options,
    /// Step a setting down (-1) or up (+1).
    Set(Setting, i8),
    /// Show text in a language ([`lang::Lang::ALL`] index).
    Language(usize),
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

/// The parts that follow the pick: on the Character tab the faction badge, the class badge, the
/// race-and-class line and the race section; in the Armory the hero's title, the race and sex
/// pickers and the appearance dials.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum HeroPart {
    Faction,
    ClassBadge,
    Line,
    Race,
    ArmoryTitle,
    ArmoryRaces,
    ArmoryDials,
}
/// The Equipment panel's slot row, filled with what the hero wears.
#[derive(Component)]
struct EquipmentRow;

/// Times the screen has been built: a refresh whose drawing went with an older screen draws again.
static BUILDS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Whether the screen was built again since a refresh last drew (its `seen`).
fn rebuilt(seen: &mut u32) -> bool {
    let now = BUILDS.load(std::sync::atomic::Ordering::Relaxed);
    if *seen == now {
        return false;
    }
    *seen = now;
    true
}

fn build_screen(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    built: Query<(), With<ScreenRoot>>,
) {
    let Some(fonts) = fonts else { return };
    if !built.is_empty() {
        return;
    }
    BUILDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
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
            // The title over the tabs.
            root.spawn((
                Chrome::Main,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
            ))
            .with_children(|h| {
                h.spawn(kit::text(
                    &f.title,
                    tr("Azeroth Survivors"),
                    56.0,
                    theme::GOLD,
                ));
                tab_row(h, f);
            });
            // The Armory's header: its title and what it holds on the left, the tabs and Done on
            // the right.
            root.spawn((
                Chrome::Armory,
                Node {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(16.0),
                    display: Display::None,
                    ..default()
                },
            ))
            .with_children(|h| {
                h.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.title, tr("Armory"), 44.0, theme::GOLD));
                    c.spawn((ArmorySub, Node::default()));
                });
                tab_row(h, f);
                kit::button(
                    h,
                    f,
                    tr("Done"),
                    None,
                    kit::ButtonLook::ACTION,
                    ScreenAction::Back,
                );
            });
            // The tabs' pages, shown in the columns' place.
            for (page, marker) in [
                (COLLECTION, 0u8),
                (TALENTS, 2),
                (DROPS, 3),
                (CODEX, 4),
                (ACHIEVEMENTS, 5),
                (REWARDS, 6),
                (OPTIONS, 7),
            ] {
                root.spawn((
                    PageRoot(page),
                    Node {
                        width: Val::Percent(100.0),
                        flex_grow: 1.0,
                        min_height: Val::Px(0.0),
                        display: Display::None,
                        ..default()
                    },
                ))
                .with_children(|pg| {
                    // The Rewards tab is its own page: the points, and what they buy.
                    if marker == 6 {
                        super::rewards::page(pg, f);
                        return;
                    }
                    kit::panel(
                        pg,
                        f,
                        Some(tr(match marker {
                            0 => "Collection",
                            2 => "Talents",
                            3 => "Drop Tables",
                            4 => "Codex",
                            5 => "Achievements",
                            6 => "Rewards",
                            _ => "Options",
                        })),
                        Node {
                            width: Val::Percent(100.0),
                            min_height: Val::Px(0.0),
                            ..default()
                        },
                        |p| {
                            kit::scroll_area(
                                p,
                                Node {
                                    flex_grow: 1.0,
                                    flex_basis: Val::Px(0.0),
                                    ..default()
                                },
                                |body| {
                                    let mut e = body.spawn(Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(8.0),
                                        ..default()
                                    });
                                    match marker {
                                        0 => e.insert(CollectionBody),
                                        2 => e.insert(TalentsBody),
                                        3 => e.insert(DropsBody),
                                        4 => e.insert(CodexBody),
                                        5 => e.insert(AchievementsBody),
                                        _ => e.insert(OptionsBody),
                                    };
                                },
                            );
                        },
                    );
                });
            }
            armory_page(root, f);
            // The three columns: the class, the hero, the zone.
            root.spawn((
                PageRoot(CHARACTER),
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(0.0),
                    column_gap: Val::Px(18.0),
                    ..default()
                },
            ))
            .with_children(|cols| {
                kit::panel(
                    cols,
                    f,
                    Some(tr("Class")),
                    Node {
                        flex_grow: 0.85,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(0.0),
                        min_height: Val::Px(0.0),
                        ..default()
                    },
                    |p| {
                        kit::scroll_area(
                            p,
                            Node {
                                flex_grow: 1.0,
                                flex_basis: Val::Px(0.0),
                                ..default()
                            },
                            |list| {
                                list.spawn((
                                    ClassColumn,
                                    Node {
                                        flex_direction: FlexDirection::Column,
                                        row_gap: Val::Px(8.0),
                                        ..default()
                                    },
                                ));
                            },
                        );
                    },
                );
                hero_column(cols, f);
                kit::panel(
                    cols,
                    f,
                    Some(tr("Zone")),
                    Node {
                        flex_grow: 0.85,
                        flex_basis: Val::Px(0.0),
                        min_width: Val::Px(0.0),
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
                Chrome::Bar,
                BottomBar,
                Node {
                    column_gap: Val::Px(14.0),
                    align_items: AlignItems::Center,
                    ..default()
                },
            ));
            root.spawn((
                Chrome::Bar,
                kit::text(
                    &f.body,
                    tr("WASD or both mouse buttons to run · 1 2 3 picks a card · \
                         R reroll · B banish · X skip · Enter to play"),
                    13.0,
                    theme::TEXT,
                ),
            ));
            root.spawn((
                Chrome::Main,
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    bottom: Val::Px(4.0),
                    ..default()
                },
            ))
            .with_children(|c| {
                c.spawn(kit::text(&f.body, tr("Made by Aethus"), 10.0, theme::DIM));
            });
        });
}

/// The tabs, the Character tab lit; the ones not built yet are greyed out.
fn tab_row(p: &mut ChildSpawnerCommands, f: &KitFonts) {
    let tabs: Vec<(&str, ScreenAction)> = TABS
        .iter()
        .enumerate()
        .map(|(i, t)| (tr(t), ScreenAction::Tab(i)))
        .collect();
    let ids = kit::tab_bar(p, f, &tabs, 0);
    for (i, id) in ids.into_iter().enumerate() {
        p.commands().entity(id).insert(TabButton(i));
        if !BUILT_TABS.contains(&i) {
            p.commands().entity(id).insert((
                kit::Disabled(true),
                Tooltip::simple(tr(TABS[i]), tr("Coming soon.")),
            ));
        }
    }
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
    catalog: Option<Res<crate::entities::CharCreate>>,
    talents: Option<Res<crate::ui_talent::Talents>>,
    session: Res<super::Session>,
    mut selection: ResMut<Selection>,
    mut run: ResMut<Run>,
    mut store: ResMut<ProfileStore>,
    mut next: ResMut<NextState<Phase>>,
    mut rng: ResMut<Rng>,
    mut page: ResMut<Page>,
    mut armory_slot: ResMut<ArmorySlot>,
    mut rig: ResMut<super::hero::CamRig>,
    mut name_field: Query<&mut TextField, With<NameField>>,
) {
    if *phase.get() != Phase::Menu {
        return;
    }
    let Some(&action) = clicks.first(&buttons) else {
        return;
    };
    match action {
        ScreenAction::Tab(i) => {
            if BUILT_TABS.contains(&i) {
                page.0 = i;
            }
        }
        ScreenAction::Armory => page.0 = ARMORY,
        ScreenAction::Options => page.0 = OPTIONS,
        ScreenAction::Set(setting, d) => {
            store.edit(|p| {
                let o = &mut p.options;
                let step = f32::from(d);
                match setting {
                    Setting::Master => o.master = (o.master + 0.1 * step).clamp(0.0, 1.0),
                    Setting::Music => o.music = (o.music + 0.1 * step).clamp(0.0, 1.0),
                    Setting::Sfx => o.sfx = (o.sfx + 0.1 * step).clamp(0.0, 1.0),
                    Setting::Sens => o.mouse_sens = (o.mouse_sens + 0.1 * step).clamp(0.2, 3.0),
                    Setting::Polish => o.polish = !o.polish,
                    Setting::Shake => o.shake = !o.shake,
                }
                // Tenths stay tenths, however many steps.
                for v in [&mut o.master, &mut o.music, &mut o.sfx, &mut o.mouse_sens] {
                    *v = (*v * 10.0).round() / 10.0;
                }
            });
        }
        ScreenAction::Language(i) => {
            if let Some(l) = lang::Lang::ALL.get(i) {
                store.edit(|p| p.options.language = l.code().into());
            }
        }
        ScreenAction::Slot(k) => armory_slot.0 = Some(k),
        ScreenAction::Turn(step) => rig.orbit += f32::from(step) * 0.5,
        ScreenAction::Claim(i) => {
            if let Some(a) = data::ACHIEVEMENTS.get(i) {
                store.edit(|p| {
                    p.claim(&a.key);
                });
            }
        }
        ScreenAction::ClaimAll => {
            store.edit(|p| {
                let keys: Vec<String> = p.unclaimed().iter().map(|a| a.key.clone()).collect();
                for k in keys {
                    p.claim(&k);
                }
            });
        }
        ScreenAction::RunOptions => {
            let zone = selection.map_def().name;
            let open = store.profile.hardest_open(zone);
            let now = selection.difficulty.min(open);
            selection.difficulty = if now >= open { 0 } else { now + 1 };
            selection.version += 1;
            menu::save_pick(&selection, &mut store);
        }
        ScreenAction::TrainPoint => {
            store.edit(|p| {
                p.train_point();
            });
        }
        ScreenAction::ResetTalents => {
            if let Some(class) = selection.class_def() {
                store.edit(|p| {
                    p.talents.remove(class.name);
                });
            }
        }
        ScreenAction::Learn(ti, ki) => {
            let (Some(class), Some((race, _)), Some(t)) = (
                selection.class_def(),
                menu::current_look(&selection),
                talents.as_deref(),
            ) else {
                return;
            };
            let trees = super::talents::trees(&t.catalog, race, class.id);
            let Some(tree) = trees.get(ti).copied() else {
                return;
            };
            let Some(talent) = tree.talents.get(ki) else {
                return;
            };
            let points = store.profile.talent_points;
            let mut spent = store
                .profile
                .talents
                .get(class.name)
                .cloned()
                .unwrap_or_default();
            if super::talents::learn(&mut spent, &trees, &tree, talent, points) {
                store.edit(|p| {
                    p.talents.insert(class.name.to_string(), spent);
                });
            }
        }
        ScreenAction::Back => page.0 = CHARACTER,
        ScreenAction::Wear(i) => {
            if let Some(item) = data::ITEMS.get(i) {
                store.edit(|p| p.equip(item));
            }
        }
        ScreenAction::Unwear(k) => {
            if let Some(&slot) = data::GearSlot::ALL.get(k) {
                store.edit(|p| p.unequip(slot));
            }
        }
        ScreenAction::Class(_) | ScreenAction::Race(_) | ScreenAction::Sex(_) => {
            let Some(creatures) = creatures.as_deref() else {
                return;
            };
            let all = menu::all_looks(creatures, &store.profile.opened_races());
            match action {
                ScreenAction::Class(c) => menu::pick_class(&mut selection, &all, c),
                ScreenAction::Race(r) => menu::pick_race(&mut selection, &all, r),
                ScreenAction::Sex(s) => menu::pick_sex(&mut selection, &all, s),
                _ => {}
            }
            // The dials held inside the new race and sex's ranges.
            if let Some((race, sex)) = menu::current_look(&selection) {
                let n = outfit::counts(catalog.as_deref().map(|c| &c.0), race, sex);
                selection.appearance = outfit::clamp(selection.appearance, n);
            }
            menu::save_pick(&selection, &mut store);
        }
        ScreenAction::Dial(..) | ScreenAction::RandomLook => {
            let Some((race, sex)) = menu::current_look(&selection) else {
                return;
            };
            let n = outfit::counts(catalog.as_deref().map(|c| &c.0), race, sex);
            let mut a = outfit::clamp(selection.appearance, n);
            match action {
                ScreenAction::Dial(i, forward) if i < a.len() => {
                    let k = n[i].max(1);
                    a[i] = if forward {
                        (a[i] + 1) % k
                    } else {
                        (a[i] + k - 1) % k
                    };
                }
                ScreenAction::RandomLook => {
                    for (d, &k) in a.iter_mut().zip(&n) {
                        *d = (rng.f32() * f32::from(k.max(1))) as u8 % k.max(1);
                    }
                }
                _ => {}
            }
            if a != selection.appearance {
                selection.appearance = a;
                selection.version += 1;
                menu::save_pick(&selection, &mut store);
            }
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
        ScreenAction::Equip(k) => {
            armory_slot.0 = Some(k);
            page.0 = ARMORY;
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
    bonuses: String,
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
    talents: Option<Res<crate::ui_talent::Talents>>,
    column: Query<Entity, With<ClassColumn>>,
    mut shown: Local<Option<ClassView>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(column)) = (fonts, column.single()) else {
        return;
    };
    let race = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .map_or(0, |l| l.1);
    let (meta, procs) = match selection.class_def() {
        Some(class) => {
            store
                .profile
                .run_bonuses(race, class, talents.as_deref().map(|t| &t.catalog))
        }
        None => (Vec::new(), Vec::new()),
    };
    let view = ClassView {
        class: selection.class,
        race,
        spells_ready: spells.is_some(),
        bonuses: format!("{meta:?}{}", procs.len()),
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
                    Some(tr(c.name)),
                    selection.class == Some(i),
                    ScreenAction::Class(i),
                );
            }
        });
        let Some(class) = selection.class_def() else {
            p.spawn(kit::text(&f.body, tr("Pick a class."), 13.0, theme::DIM));
            return;
        };
        let hero = Vitals::new(class, race).with_meta(meta, &procs);
        let stats = hero.stats();
        p.spawn(kit::text(
            &f.body,
            tr(class.name),
            20.0,
            hud::rgb(class.color),
        ));
        p.spawn(kit::text(&f.body, class.blurb, 13.0, theme::TEXT));
        p.spawn(Node {
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            kit::stat_tile(r, f, tr("Health"), &format!("{:.0}", stats.max_hp));
            let start = data::ability(class.start);
            kit::stat_tile(
                r,
                f,
                tr("Starts with"),
                &hud::spell_name(spells, start.spell, start.key),
            );
            kit::stat_tile(
                r,
                f,
                tr("Zones open"),
                &trf(
                    "{} of {}",
                    &[
                        &open_zones(&store).to_string(),
                        &data::MAPS.len().to_string(),
                    ],
                ),
            );
        });
        p.spawn(kit::text(&f.body, tr("SPELL POOL"), 10.0, theme::DIM));
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
    let owned = Owned::learned(def, &[], 0.0);
    hud::ability_tip(&owned, stats, spells)
}

/// The Alliance's races; the rest are the Horde's.
fn alliance(race: u8) -> bool {
    matches!(race, 1 | 3 | 4 | 7)
}

/// The races in the character-create order.
const RACES: [u8; 8] = [1, 3, 7, 4, 2, 5, 6, 8];

/// The portrait's size, and how far its frame sits in from the column's edge.
const PORTRAIT: Vec2 = Vec2::new(150.0, 210.0);
const PORTRAIT_INSET: f32 = 22.0;

/// The portrait's look: no fill, so the hero shows through; a brass rim that lights on hover.
const STAGE_LOOK: kit::ButtonLook = kit::ButtonLook {
    rest: Color::NONE,
    hot: Color::srgba(1.0, 0.82, 0.0, 0.05),
    edge: Color::srgba(0.62, 0.5, 0.26, 0.8),
    edge_selected: theme::GOLD,
};

/// A panel's fill, for the pieces the hero column is built from.
fn panel_fill() -> BackgroundColor {
    BackgroundColor(theme::PANEL_TOP)
}

/// The middle column: the hero's portrait (a click opens the Armory) beside its name, race and
/// class; then the equipment worn and the collection, the Armory and Talents, and the race.
///
/// A fill would hide the hero the camera frames in the portrait, so the column is not one panel
/// but pieces of fill around the portrait's hole, inside one brass frame.
fn hero_column(cols: &mut ChildSpawnerCommands, f: &KitFonts) {
    let r = Val::Px(6.0);
    cols.spawn((
        Node {
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            min_width: Val::Px(0.0),
            flex_direction: FlexDirection::Column,
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(r),
            ..default()
        },
        BorderColor::all(theme::BRASS),
    ))
    .with_children(|c| {
        // The top band: fill, the portrait's hole, fill with the name and the line.
        c.spawn(Node {
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|band| {
            band.spawn((
                Node {
                    width: Val::Px(PORTRAIT_INSET),
                    border_radius: BorderRadius::top_left(r),
                    ..default()
                },
                panel_fill(),
            ));
            band.spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|col| {
                col.spawn((
                    Node {
                        height: Val::Px(PORTRAIT_INSET),
                        ..default()
                    },
                    panel_fill(),
                ));
                col.spawn((
                    StageFrame { turning: true },
                    Button,
                    STAGE_LOOK,
                    kit::Selected(false),
                    kit::Disabled(false),
                    ScreenAction::Armory,
                    Tooltip::simple(tr("Armory"), tr("Race, look, gear and drop tables.")),
                    Node {
                        width: Val::Px(PORTRAIT.x),
                        height: Val::Px(PORTRAIT.y),
                        border: UiRect::all(Val::Px(1.5)),
                        border_radius: BorderRadius::all(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(STAGE_LOOK.edge),
                ))
                .with_children(|s| {
                    s.spawn((
                        HeroPart::Faction,
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Px(-10.0),
                            top: Val::Px(-12.0),
                            ..default()
                        },
                    ));
                    s.spawn((
                        HeroPart::ClassBadge,
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(-14.0),
                            bottom: Val::Px(-14.0),
                            ..default()
                        },
                    ));
                });
                // Under the portrait, should the line beside it run taller.
                col.spawn((
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                    panel_fill(),
                ));
            });
            band.spawn((
                Node {
                    flex_grow: 1.0,
                    min_width: Val::Px(0.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    padding: UiRect {
                        left: Val::Px(PORTRAIT_INSET),
                        right: Val::Px(PORTRAIT_INSET),
                        top: Val::Px(PORTRAIT_INSET + 26.0),
                        bottom: Val::Px(8.0),
                    },
                    border_radius: BorderRadius::top_right(r),
                    ..default()
                },
                panel_fill(),
            ))
            .with_children(|info| {
                info.spawn(Node {
                    align_items: AlignItems::Stretch,
                    ..default()
                })
                .with_children(|n| {
                    kit::text_field(
                        n,
                        f,
                        "",
                        300.0,
                        names::MAX_NAME,
                        names::name_char,
                        NameField,
                    );
                    kit::button(
                        n,
                        f,
                        tr("Random"),
                        None,
                        kit::ButtonLook::PLAIN,
                        ScreenAction::RandomName,
                    );
                });
                info.spawn((
                    HeroPart::Line,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(6.0),
                        ..default()
                    },
                ));
            });
        });
        // Under the band: one fill to the bottom.
        c.spawn((
            Node {
                flex_grow: 1.0,
                min_height: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(10.0),
                padding: UiRect {
                    left: Val::Px(PORTRAIT_INSET),
                    right: Val::Px(PORTRAIT_INSET),
                    top: Val::Px(18.0),
                    bottom: Val::Px(14.0),
                },
                border_radius: BorderRadius::bottom(r),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::percent(theme::PANEL_TOP, 0.0),
                ColorStop::percent(theme::PANEL_BOTTOM, 100.0),
            ])),
        ))
        .with_children(|fill| {
            // Scrolled, not clipped or spilled: on a short screen the racials would otherwise run
            // under the bottom bar.
            kit::scroll_area(
                fill,
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    row_gap: Val::Px(10.0),
                    ..default()
                },
                |p| {
                    kit::heading(p, f, tr("Equipment"));
                    p.spawn((
                        EquipmentRow,
                        Node {
                            column_gap: Val::Px(6.0),
                            row_gap: Val::Px(6.0),
                            justify_content: JustifyContent::Center,
                            flex_wrap: FlexWrap::Wrap,
                            ..default()
                        },
                    ));
                    p.spawn((
                        CollectionBar,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                    ));
                    p.spawn((
                        HeroButtons,
                        Node {
                            column_gap: Val::Px(12.0),
                            ..default()
                        },
                    ));
                    p.spawn((
                        HeroPart::Race,
                        Node {
                            column_gap: Val::Px(14.0),
                            margin: UiRect::top(Val::Px(8.0)),
                            ..default()
                        },
                    ));
                },
            );
        });
    });
}

/// The Armory, in the columns' place: the race, sex and gear on the left, the hero on its stage
/// in the middle, the appearance dials and the pieces owned for the slot picked on the right.
fn armory_page(root: &mut ChildSpawnerCommands, f: &KitFonts) {
    root.spawn((
        PageRoot(ARMORY),
        Node {
            width: Val::Percent(100.0),
            flex_grow: 1.0,
            min_height: Val::Px(0.0),
            column_gap: Val::Px(22.0),
            display: Display::None,
            ..default()
        },
    ))
    .with_children(|cols| {
        let side = |grow: f32| Node {
            flex_grow: grow,
            flex_basis: Val::Px(0.0),
            min_width: Val::Px(0.0),
            min_height: Val::Px(0.0),
            ..default()
        };
        kit::panel(cols, f, Some(tr("Appearance")), side(0.9), |p| {
            p.spawn((
                HeroPart::ArmoryRaces,
                Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    ..default()
                },
            ));
            kit::heading(p, f, tr("Equipment"));
            kit::scroll_area(
                p,
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    ..default()
                },
                |list| {
                    list.spawn((
                        ArmorySlots,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            ..default()
                        },
                    ));
                },
            );
            p.spawn((
                ArmoryNote,
                Node {
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
            ));
        });
        // The stage: a frame with its fill faded out in the middle, so the hero shows through.
        let edge = theme::PANEL_BOTTOM;
        cols.spawn((
            Node {
                flex_grow: 2.2,
                flex_basis: Val::Px(0.0),
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(14.0)),
                row_gap: Val::Px(6.0),
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom(vec![
                ColorStop::percent(edge, 0.0),
                ColorStop::percent(edge.with_alpha(0.0), 16.0),
                ColorStop::percent(edge.with_alpha(0.0), 84.0),
                ColorStop::percent(edge, 100.0),
            ])),
            BorderColor::all(theme::BRASS),
        ))
        .with_children(|p| {
            p.spawn((
                HeroPart::ArmoryTitle,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
            ));
            p.spawn((
                StageFrame { turning: false },
                StageDrag,
                Button,
                Node {
                    width: Val::Percent(100.0),
                    flex_grow: 1.0,
                    min_height: Val::Px(200.0),
                    ..default()
                },
            ));
            p.spawn(Node {
                column_gap: Val::Px(16.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                let turn = |r: &mut ChildSpawnerCommands, art: &str, step: i8| {
                    kit::icon_button(
                        r,
                        f,
                        IconSource::Texture(format!(
                            "Interface\\Buttons\\UI-Rotation{art}-Button-Up"
                        )),
                        24.0,
                        None,
                        false,
                        ScreenAction::Turn(step),
                    );
                };
                turn(r, "Left", -1);
                r.spawn(kit::text(
                    &f.body,
                    tr("Drag the hero to turn"),
                    14.0,
                    theme::TEXT,
                ));
                turn(r, "Right", 1);
            });
        });
        kit::panel(cols, f, Some(tr("Features")), side(0.9), |p| {
            p.spawn((
                HeroPart::ArmoryDials,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(6.0),
                    ..default()
                },
            ));
            kit::scroll_area(
                p,
                Node {
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    margin: UiRect::top(Val::Px(10.0)),
                    ..default()
                },
                |list| {
                    list.spawn((
                        ArmoryPicker,
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(8.0),
                            ..default()
                        },
                    ));
                },
            );
        });
    });
}

/// The Armory and Talents buttons, with the talent points trained.
fn refresh_hero_buttons(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    store: Res<ProfileStore>,
    row: Query<Entity, With<HeroButtons>>,
    mut shown: Local<Option<u32>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(row)) = (fonts, row.single()) else {
        return;
    };
    let points = store.profile.talent_points;
    if *shown == Some(points) {
        return;
    }
    *shown = Some(points);
    let f = &*fonts;
    commands.entity(row).despawn_related::<Children>();
    commands.entity(row).with_children(|r| {
        let points = trf(
            "{} of {} points",
            &[&points.to_string(), &super::talents::MAX_POINTS.to_string()],
        );
        for (label, sub, action) in [
            (
                "Armory",
                "Race, look, gear and drop tables",
                ScreenAction::Armory,
            ),
            ("Talents", points.as_str(), ScreenAction::Tab(TALENTS)),
        ] {
            let b = kit::button(
                r,
                f,
                tr(label),
                Some(tr(sub)),
                kit::ButtonLook::ACTION,
                action,
            );
            r.commands().entity(b).entry::<Node>().and_modify(|mut n| {
                n.flex_grow = 1.0;
                n.flex_basis = Val::Px(0.0);
                n.padding = UiRect::axes(Val::Px(14.0), Val::Px(10.0));
            });
        }
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

/// What the hero's parts show, to rebuild them only when that changes.
#[derive(PartialEq, Clone, Debug)]
struct HeroView {
    class: Option<usize>,
    look: Option<(u8, u8)>,
    looks_known: bool,
    appearance: outfit::Appearance,
    dials_known: bool,
    strings_known: bool,
    name: String,
    /// The races opened to every class in the Rewards tab.
    races: Vec<u8>,
}

/// A dial's label as the create screen words it for the race and sex ("Piercings", "Tusks"), from
/// the glue strings; the built-in caption without them.
fn dial_label(
    i: usize,
    strings: Option<&crate::glue_strings::GlueStrings>,
    catalog: Option<&crate::entities::CharCreate>,
    race: u8,
    sex: u8,
) -> String {
    let fallback = [
        "Skin Color",
        "Face",
        "Hair Style",
        "Hair Color",
        "Facial Hair",
    ][i.min(4)];
    let Some(strings) = strings else {
        return fallback.to_string();
    };
    let cat = catalog.map(|c| &c.0);
    let hair = cat
        .and_then(|c| c.hair_customization(race))
        .unwrap_or("NORMAL");
    let facial = cat
        .and_then(|c| c.facial_hair_customization(race, sex))
        .unwrap_or("NORMAL");
    let key = match i {
        0 => "CHAR_CUSTOMIZATION1_DESC".to_string(),
        1 => "CHAR_CUSTOMIZATION2_DESC".to_string(),
        2 => format!("HAIR_{hair}_STYLE"),
        3 => format!("HAIR_{hair}_COLOR"),
        _ => format!("FACIAL_HAIR_{facial}"),
    };
    strings.text(&key, fallback).to_string()
}

/// The parts that follow the pick, on the Character tab and in the Armory; the name too, when
/// the profile has none yet.
#[allow(clippy::too_many_arguments)]
fn refresh_hero_card(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    creatures: Option<Res<crate::entities::Creatures>>,
    catalog: Option<Res<crate::entities::CharCreate>>,
    strings: Option<Res<crate::glue_strings::GlueStrings>>,
    mut store: ResMut<ProfileStore>,
    mut rng: ResMut<Rng>,
    mut name: Query<&mut TextField, With<NameField>>,
    parts: Query<(Entity, &HeroPart)>,
    mut shown: Local<Option<HeroView>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
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
        appearance: selection.appearance,
        dials_known: catalog.is_some(),
        strings_known: strings.is_some(),
        name: store.profile.hero.name.clone(),
        races: store.profile.opened_races(),
    };
    if shown.as_ref() == Some(&view) {
        return;
    }
    let part = |want: HeroPart| parts.iter().find(|(_, p)| **p == want).map(|(e, _)| e);
    let (
        Some(faction),
        Some(badge),
        Some(line),
        Some(race_section),
        Some(title),
        Some(races),
        Some(dials),
    ) = (
        part(HeroPart::Faction),
        part(HeroPart::ClassBadge),
        part(HeroPart::Line),
        part(HeroPart::Race),
        part(HeroPart::ArmoryTitle),
        part(HeroPart::ArmoryRaces),
        part(HeroPart::ArmoryDials),
    )
    else {
        return;
    };
    *shown = Some(view);
    let f = &*fonts;
    let opened = &store.profile.opened_races();
    let all = creatures
        .as_deref()
        .map(|c| menu::all_looks(c, opened))
        .unwrap_or_default();
    for e in [faction, badge, line, race_section, title, races, dials] {
        commands.entity(e).despawn_related::<Children>();
    }
    let Some((race, sex)) = look else {
        return;
    };
    let class = selection.class_def();
    let race_name = tr(crate::char_select::race_name(race));
    let sex_name = tr(if sex == 1 { "Female" } else { "Male" });
    let race_icon = |race_id: u8| {
        crate::glue::art::race_tc(race_id, sex).map(|tc| {
            IconSource::Atlas(
                "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Races".into(),
                tc,
            )
        })
    };

    commands.entity(faction).with_children(|b| {
        let (label, color) = faction_tag(race);
        b.spawn((
            Node {
                padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(9.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.03, 0.02, 0.9)),
            BorderColor::all(color),
        ))
        .with_children(|c| {
            c.spawn(kit::text(&f.body, label, 11.0, color));
        });
    });

    if let Some(class) = class {
        commands.entity(badge).with_children(|b| {
            b.spawn((
                Node {
                    padding: UiRect::all(Val::Px(3.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::MAX,
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.03, 0.02, 0.95)),
                BorderColor::all(theme::BRASS),
            ))
            .with_children(|c| {
                kit::icon_image(c, &class_icon(class), 44.0);
            });
        });
    }

    commands.entity(line).with_children(|l| {
        l.spawn(Node {
            column_gap: Val::Px(7.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn(kit::text(&f.body, race_name, 22.0, theme::TEXT));
            if let Some(class) = class {
                r.spawn(kit::text(&f.body, class.name, 22.0, hud::rgb(class.color)));
            }
        });
        l.spawn(kit::text(&f.body, sex_name, 16.0, theme::DIM));
    });

    commands.entity(race_section).with_children(|r| {
        if let Some(icon) = race_icon(race) {
            kit::icon(r, icon, 48.0, None);
        }
        r.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(kit::text(&f.title, race_name, 24.0, theme::GOLD));
            for def in data::racials(race) {
                c.spawn((
                    Node {
                        column_gap: Val::Px(8.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    racial_tip(def),
                    Interaction::default(),
                    Button,
                ))
                .with_children(|row| {
                    kit::icon(row, IconSource::Spell(def.spell), 18.0, None);
                    row.spawn(kit::text(
                        &f.body,
                        format!("{}: {}", def.name, def.text),
                        15.0,
                        theme::TEXT,
                    ));
                });
            }
        });
    });

    commands.entity(title).with_children(|t| {
        t.spawn(kit::text(
            &f.title,
            &store.profile.hero.name,
            40.0,
            theme::GOLD,
        ));
        t.spawn(Node {
            column_gap: Val::Px(6.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn(kit::text(
                &f.body,
                format!("{sex_name} {race_name}"),
                17.0,
                theme::TEXT,
            ));
            if let Some(class) = class {
                r.spawn(kit::text(
                    &f.body,
                    tr(class.name),
                    17.0,
                    hud::rgb(class.color),
                ));
            }
        });
    });

    commands.entity(races).with_children(|p| {
        p.spawn(Node {
            column_gap: Val::Px(12.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|sides| {
            for side_alliance in [true, false] {
                sides
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|side| {
                        let tag = faction_tag(if side_alliance { 1 } else { 2 });
                        side.spawn(kit::text(&f.body, tag.0, 11.0, tag.1));
                        side.spawn(Node {
                            column_gap: Val::Px(3.0),
                            ..default()
                        })
                        .with_children(|r| {
                            for race_id in
                                RACES.into_iter().filter(|&r| alliance(r) == side_alliance)
                            {
                                race_button(
                                    r,
                                    f,
                                    race_id,
                                    race,
                                    &all,
                                    opened.contains(&race_id),
                                    &selection,
                                    race_icon(race_id),
                                );
                            }
                        });
                    });
            }
        });
        p.spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn(Node {
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|c| {
                c.spawn(kit::text(&f.body, race_name, 20.0, theme::TEXT));
            });
            for (s, label) in [(0u8, "Male"), (1, "Female")] {
                let b = kit::button(
                    r,
                    f,
                    tr(label),
                    None,
                    kit::ButtonLook::PLAIN,
                    ScreenAction::Sex(s),
                );
                r.commands().entity(b).insert(kit::Selected(s == sex));
            }
        });
    });

    // The appearance dials, each a row with its arrows; a dial the race has one of is left out.
    let counts = outfit::counts(catalog.as_deref().map(|c| &c.0), race, sex);
    let a = outfit::clamp(selection.appearance, counts);
    commands.entity(dials).with_children(|p| {
        for i in 0..outfit::DIALS.len() {
            if counts[i] <= 1 {
                continue;
            }
            let label = dial_label(i, strings.as_deref(), catalog.as_deref(), race, sex);
            dial_row(p, f, i, &label, a[i], counts[i]);
        }
        if counts.iter().any(|&n| n > 1) {
            kit::button(
                p,
                f,
                tr("Randomize"),
                None,
                kit::ButtonLook::PLAIN,
                ScreenAction::RandomLook,
            );
        }
    });
}

/// "ALLIANCE" in blue or "HORDE" in red, for a race.
fn faction_tag(race: u8) -> (&'static str, Color) {
    if alliance(race) {
        ("ALLIANCE", Color::srgb(0.3, 0.55, 1.0))
    } else {
        ("HORDE", Color::srgb(0.95, 0.25, 0.2))
    }
}

/// A race's button in the Armory, lit when picked, hidden when the client has no body for it.
#[allow(clippy::too_many_arguments)]
fn race_button(
    r: &mut ChildSpawnerCommands,
    f: &KitFonts,
    race_id: u8,
    race: u8,
    all: &[Vec<(u32, u8, u8)>],
    opened: bool,
    selection: &Selection,
    icon: Option<IconSource>,
) {
    let Some(icon) = icon else { return };
    let playable = all.is_empty() || menu::race_playable(all, race_id);
    let e = kit::icon_button(
        r,
        f,
        icon,
        34.0,
        None,
        race_id == race,
        ScreenAction::Race(race_id),
    );
    if !playable {
        r.commands().entity(e).insert(Visibility::Hidden);
    }
    let name = tr(crate::char_select::race_name(race_id));
    let class = selection.class_def();
    // The client's looks decide; without them, vanilla's own pairs.
    let keeps_class = if all.is_empty() {
        opened || class.is_none_or(|c| data::race_classes(race_id).contains(&c.id))
    } else {
        selection
            .class
            .and_then(|c| all.get(c))
            .is_some_and(|looks| looks.iter().any(|l| l.1 == race_id))
    };
    let note = if keeps_class {
        trf("Play as a {}.", &[tr(name)])
    } else {
        trf("Picking {} changes your class.", &[tr(name)])
    };
    let mut tip = vec![
        TipLine::new(name, Color::WHITE).header(),
        TipLine::new(note, theme::TEXT),
    ];
    if opened {
        tip.push(TipLine::new(tr("Opened to every class."), theme::GREEN));
    } else if !keeps_class {
        tip.push(TipLine::new(
            tr("Open it to every class in the Rewards tab."),
            theme::DIM,
        ));
    }
    r.commands().entity(e).insert(Tooltip(tip));
}

/// One appearance dial: its arrows either side of its label, "7 / 9", and a dot per choice.
fn dial_row(p: &mut ChildSpawnerCommands, f: &KitFonts, i: usize, label: &str, at: u8, count: u8) {
    p.spawn((
        Node {
            width: Val::Percent(100.0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(theme::INSET),
        BorderColor::all(Color::srgba(0.4, 0.33, 0.2, 0.5)),
    ))
    .with_children(|row| {
        let arrow = |row: &mut ChildSpawnerCommands, art: &str, forward: bool| {
            kit::icon_button(
                row,
                f,
                IconSource::Texture(format!(
                    "Interface\\Glues\\Common\\Glue-{art}Arrow-Button-Up"
                )),
                28.0,
                None,
                false,
                ScreenAction::Dial(i, forward),
            );
        };
        arrow(row, "Left", false);
        row.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(kit::text(&f.body, label.to_uppercase(), 11.0, theme::TEXT));
            c.spawn(kit::text(
                &f.number,
                format!("{} / {count}", at + 1),
                20.0,
                theme::TEXT,
            ));
            if count <= 24 {
                c.spawn(Node {
                    column_gap: Val::Px(4.0),
                    ..default()
                })
                .with_children(|dots| {
                    for k in 0..count {
                        dots.spawn((
                            Node {
                                width: Val::Px(5.0),
                                height: Val::Px(5.0),
                                border_radius: BorderRadius::MAX,
                                ..default()
                            },
                            BackgroundColor(if k == at {
                                theme::GOLD
                            } else {
                                theme::DIM.with_alpha(0.45)
                            }),
                        ));
                    }
                });
            }
        });
        arrow(row, "Right", true);
    });
}

/// A racial's tooltip: its name, what it does, and whether it is always on or fires.
fn racial_tip(def: &data::RacialDef) -> Tooltip {
    let kind = tr(if def.proc.is_some() {
        "Fires on its own"
    } else {
        "Always on"
    });
    Tooltip(vec![
        TipLine::new(def.name, Color::WHITE)
            .header()
            .right(tr("Racial"), theme::DIM),
        TipLine::new(kind, theme::DIM),
        TipLine::new(def.text, theme::GREEN),
    ])
}

/// A piece of gear's tooltip: its name in its quality's colour, the slot, what it gives, where
/// it drops, and how to change it.
pub(super) fn gear_tip(item: &data::ItemDef, hint: String) -> Tooltip {
    let mut lines = vec![
        TipLine::new(item.name.clone(), hud::rgb(item.rarity.color())).header(),
        TipLine::new(tr(item.slot.name()), Color::WHITE).right(tr(item.rarity.name()), theme::DIM),
    ];
    for b in &item.bonuses {
        lines.push(TipLine::new(format!("+{}", bonus_text(b)), theme::GREEN));
    }
    let boss = data::roster(item.zone).bosses[item.boss].name;
    lines.push(TipLine::new(
        trf("Dropped by {}, {}", &[boss, tr(data::MAPS[item.zone].name)]),
        theme::DIM,
    ));
    lines.push(TipLine::new(hint, theme::GOLD));
    Tooltip(lines)
}

/// One bonus as the tooltips word it: "4% max health", "1.5 armor".
pub(super) fn bonus_text(b: &data::Bonus) -> String {
    use data::{Amount, Stat};
    let what = match b.stat {
        Stat::Might => "damage",
        Stat::MaxHp => "max health",
        Stat::Speed => "movement speed",
        Stat::Regen => "health per second",
        Stat::Haste => "cooldown recovery",
        Stat::Area => "area of effect",
        Stat::Magnet => "pickup range",
        Stat::Growth => "experience",
        Stat::Armor => "damage blocked per hit",
        Stat::Crit => "critical strike",
        Stat::DamageReduction => "damage reduction",
        Stat::Gold => "gold",
    };
    let fraction = matches!(
        b.stat,
        Stat::Might | Stat::Haste | Stat::Area | Stat::Growth | Stat::Crit | Stat::DamageReduction
    );
    // One decimal, dropped when it is a zero: "25", "1.5".
    let n = |v: f32| {
        let t = format!("{v:.1}");
        t.strip_suffix(".0").map(str::to_string).unwrap_or(t)
    };
    let what = tr(what);
    match b.amount {
        Amount::Percent(v) => format!("{}% {what}", n(v * 100.0)),
        Amount::Flat(v) if fraction => format!("{}% {what}", n(v * 100.0)),
        Amount::Flat(v) => format!("{} {what}", n(v)),
    }
}

/// What the Equipment parts show, to rebuild them only when that changes.
#[derive(PartialEq, Clone, Debug)]
struct EquipmentView {
    class: Option<usize>,
    look: Option<(u8, u8)>,
    ready: (bool, bool),
    worn: Vec<String>,
    owned: usize,
    slot: Option<usize>,
}

/// "+25 max health · +1% critical strike" for the gear worn, or what to do about wearing none.
fn worn_summary(gear: &[&data::ItemDef], none: &str) -> (String, Color) {
    let bonuses: Vec<data::Bonus> = gear
        .iter()
        .flat_map(|i| i.bonuses.iter().cloned())
        .collect();
    if bonuses.is_empty() {
        return (none.to_string(), theme::TEXT);
    }
    let lines: Vec<String> = data::sum_bonuses(&bonuses)
        .iter()
        .map(|b| format!("+{}", bonus_text(b)))
        .collect();
    (lines.join(" · "), theme::GREEN)
}

/// The ten slots, on the Character tab and in the Armory: the gear worn there with its icon,
/// else the starting outfit's piece, else the paper doll's empty art. A slot clicked on the
/// Character tab opens the Armory on it; in the Armory, the pieces owned for the slot picked
/// show on the right, and a click wears one.
#[allow(clippy::too_many_arguments)]
fn refresh_equipment(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    store: Res<ProfileStore>,
    picked: Res<ArmorySlot>,
    catalog: Option<Res<crate::entities::CharCreate>>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    row: Query<Entity, With<EquipmentRow>>,
    slots: Query<Entity, With<ArmorySlots>>,
    picker: Query<Entity, With<ArmoryPicker>>,
    note: Query<Entity, With<ArmoryNote>>,
    mut shown: Local<Option<EquipmentView>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(row), Ok(slots), Ok(picker), Ok(note)) = (
        fonts,
        row.single(),
        slots.single(),
        picker.single(),
        note.single(),
    ) else {
        return;
    };
    let look = menu::current_look(&selection);
    let gear = store.profile.gear();
    let view = EquipmentView {
        class: selection.class,
        look,
        ready: (catalog.is_some(), displays.is_some()),
        worn: gear.iter().map(|i| i.name.clone()).collect(),
        owned: store.profile.inventory.len(),
        slot: picked.0,
    };
    if shown.as_ref() == Some(&view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let class = selection.class_def();
    let outfit = match (class, look) {
        (Some(class), Some((race, sex))) => match outfit::body(
            class,
            race,
            sex,
            selection.appearance,
            0,
            &[],
            None,
            catalog.as_deref().map(|c| &c.0),
            None,
        ) {
            outfit::Body::Player { worn, .. } => worn,
            outfit::Body::Trainer { .. } => Vec::new(),
        },
        _ => Vec::new(),
    };
    let icon_of = |display: u32| -> Option<String> {
        displays.as_deref()?.catalog.get(display)?.icon.clone()
    };
    let item_icon = |item: &data::ItemDef| {
        displays
            .as_deref()
            .and_then(|d| outfit::gear_icon(item, class, &d.catalog))
            .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".to_string())
    };
    // Each slot: its name, what it wears, the icon it shows, and its tooltip's last lines.
    let shown_slots: Vec<(&str, Option<&data::ItemDef>, String, Tooltip)> = outfit::SHOWN
        .into_iter()
        .enumerate()
        .map(|(k, (label, art, slot))| {
            let gear_slot = data::GearSlot::ALL[k];
            let worn_item = gear.iter().find(|i| i.slot == gear_slot).copied();
            let owned = store.profile.owned_for(gear_slot).len();
            let empty = format!("Interface\\PaperDoll\\UI-PaperDoll-Slot-{art}");
            match worn_item {
                Some(item) => (
                    label,
                    Some(item),
                    displays
                        .as_deref()
                        .and_then(|d| outfit::gear_icon(item, class, &d.catalog))
                        .unwrap_or(empty),
                    gear_tip(
                        item,
                        if owned > 1 {
                            trf("{} owned for this slot.", &[&owned.to_string()])
                        } else {
                            tr("The only one you own for this slot.").to_string()
                        },
                    ),
                ),
                None => {
                    let piece = outfit.iter().find(|w| w.slot == slot);
                    let note = match (piece, owned) {
                        (_, n) if n > 0 => {
                            TipLine::new(format!("{n} found: pick one to wear."), theme::GOLD)
                        }
                        (Some(_), _) => TipLine::new(tr("Starting gear"), theme::TEXT),
                        (None, _) => TipLine::new(tr("Empty"), theme::DIM),
                    };
                    (
                        label,
                        None,
                        piece.and_then(|w| icon_of(w.display)).unwrap_or(empty),
                        Tooltip(vec![TipLine::new(label, Color::WHITE).header(), note]),
                    )
                }
            }
        })
        .collect();

    commands.entity(row).despawn_related::<Children>();
    commands.entity(row).with_children(|r| {
        for (k, (_, worn_item, icon, tip)) in shown_slots.iter().enumerate() {
            let b = kit::icon_button(
                r,
                f,
                IconSource::Texture(icon.clone()),
                40.0,
                None,
                worn_item.is_some(),
                ScreenAction::Equip(k),
            );
            r.commands().entity(b).insert(Tooltip(tip.0.clone()));
            if let Some(item) = worn_item {
                r.commands()
                    .entity(b)
                    .insert(BorderColor::all(hud::rgb(item.rarity.color())));
            }
        }
    });

    commands.entity(slots).despawn_related::<Children>();
    commands.entity(slots).with_children(|list| {
        for (k, (label, worn_item, icon, tip)) in shown_slots.iter().enumerate() {
            let b = list
                .spawn((
                    Button,
                    kit::ButtonLook::PLAIN,
                    kit::Selected(picked.0 == Some(k)),
                    kit::Disabled(false),
                    ScreenAction::Slot(k),
                    Tooltip(tip.0.clone()),
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(10.0),
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(theme::BUTTON),
                    BorderColor::all(theme::BRASS),
                ))
                .id();
            list.commands().entity(b).with_children(|r| {
                let i = kit::icon(r, IconSource::Texture(icon.clone()), 36.0, None);
                if let Some(item) = worn_item {
                    r.commands()
                        .entity(i)
                        .insert(BorderColor::all(hud::rgb(item.rarity.color())));
                }
                r.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    min_width: Val::Px(0.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.body, label.to_uppercase(), 10.0, theme::TEXT));
                    match worn_item {
                        Some(item) => c.spawn(kit::text(
                            &f.body,
                            item.name.clone(),
                            15.0,
                            hud::rgb(item.rarity.color()),
                        )),
                        None => c.spawn(kit::text(&f.body, tr("Empty"), 15.0, theme::DIM)),
                    };
                });
            });
        }
    });

    commands.entity(picker).despawn_related::<Children>();
    commands.entity(picker).with_children(|p| {
        let Some(k) = picked.0.filter(|&k| k < data::GearSlot::ALL.len()) else {
            p.spawn(kit::text(
                &f.body,
                tr("Pick a slot on the left to choose what the hero shows there."),
                13.0,
                theme::TEXT,
            ));
            return;
        };
        let slot = data::GearSlot::ALL[k];
        let worn = gear.iter().find(|i| i.slot == slot).copied();
        let owned = store.profile.owned_for(slot);
        p.spawn(kit::text(&f.title, slot.name(), 20.0, theme::GOLD));
        p.spawn(Node {
            column_gap: Val::Px(6.0),
            row_gap: Val::Px(6.0),
            align_items: AlignItems::Center,
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|r| {
            let none = kit::button(
                r,
                f,
                tr("None"),
                None,
                kit::ButtonLook::PLAIN,
                ScreenAction::Unwear(k),
            );
            r.commands()
                .entity(none)
                .insert(kit::Selected(worn.is_none()));
            for item in &owned {
                let Some(index) = data::ITEMS.iter().position(|i| i == *item) else {
                    continue;
                };
                let b = kit::icon_button(
                    r,
                    f,
                    IconSource::Texture(item_icon(item)),
                    36.0,
                    None,
                    worn == Some(*item),
                    ScreenAction::Wear(index),
                );
                let n = store
                    .profile
                    .inventory
                    .get(&item.name)
                    .copied()
                    .unwrap_or(0);
                let hint = if worn == Some(*item) {
                    trf("Worn · found {}×", &[&n.to_string()])
                } else {
                    trf("Click to wear it · found {}×", &[&n.to_string()])
                };
                r.commands().entity(b).insert(gear_tip(item, hint));
            }
        });
        if owned.is_empty() {
            p.spawn(kit::text(
                &f.body,
                tr("Nothing found for this slot yet: the bosses drop it."),
                12.0,
                theme::DIM,
            ));
        } else {
            p.spawn(kit::text(
                &f.body,
                tr("What you wear shows on your hero and adds to every run."),
                12.0,
                theme::DIM,
            ));
        }
    });

    commands.entity(note).despawn_related::<Children>();
    commands.entity(note).with_children(|n| {
        let (text, color) = worn_summary(
            &gear,
            tr("Nothing worn adds anything yet: bosses drop the class's gear."),
        );
        n.spawn(kit::text(&f.body, text, 13.0, color));
    });
}

/// The page picked shows in the columns' place, and its tab is lit.
fn show_pages(
    page: Res<Page>,
    mut roots: Query<(&PageRoot, &mut Node), Without<Chrome>>,
    mut chrome: Query<(&Chrome, &mut Node), Without<PageRoot>>,
    mut tabs: Query<(&TabButton, &mut kit::Selected)>,
) {
    if !page.is_changed() {
        return;
    }
    let armory = page.0 == ARMORY;
    for (part, mut node) in &mut chrome {
        let on = match part {
            Chrome::Main | Chrome::Bar => !armory,
            Chrome::Armory => armory,
        };
        let want = if on { Display::Flex } else { Display::None };
        if node.display != want {
            node.display = want;
        }
    }
    for (root, mut node) in &mut roots {
        let want = if root.0 == page.0 {
            Display::Flex
        } else {
            Display::None
        };
        if node.display != want {
            node.display = want;
        }
    }
    for (tab, mut selected) in &mut tabs {
        let want = tab.0 == page.0 || (page.0 == ARMORY && tab.0 == CHARACTER);
        if selected.0 != want {
            selected.0 = want;
        }
    }
}

/// "+25 max health · +1% critical strike": the collection's bonuses added up, for one line.
fn bonus_summary(bonuses: &[data::Bonus]) -> String {
    let lines: Vec<String> = data::sum_bonuses(bonuses)
        .iter()
        .map(|b| format!("+{}", bonus_text(b)))
        .collect();
    if lines.is_empty() {
        tr("Find gear to grow your collection: every piece found adds a bonus to every run.").into()
    } else {
        lines.join(" · ")
    }
}

/// The Collection tab, every zone's five pieces, found or not; and the bar under the equipment.
#[allow(clippy::too_many_arguments)]
fn refresh_collection(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    store: Res<ProfileStore>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    body: Query<Entity, With<CollectionBody>>,
    bar: Query<Entity, With<CollectionBar>>,
    sub: Query<Entity, With<ArmorySub>>,
    mut shown: Local<Option<(usize, Option<usize>, bool, Vec<String>)>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(body), Ok(bar), Ok(sub)) =
        (fonts, body.single(), bar.single(), sub.single())
    else {
        return;
    };
    let collected = store.profile.collected();
    let gear = store.profile.gear();
    let view = (
        collected.len(),
        selection.class,
        displays.is_some(),
        gear.iter().map(|i| i.name.clone()).collect(),
    );
    if shown.as_ref() == Some(&view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let total = data::ITEMS.len();
    let summary = bonus_summary(&store.profile.collection_bonuses());
    let progress = || kit::Progress {
        value: collected.len() as f32,
        max: total as f32,
    };
    commands.entity(bar).despawn_related::<Children>();
    commands.entity(bar).with_children(|b| {
        kit::progress_bar(b, f, tr("Collection"), progress());
        let (text, color) = worn_summary(
            &gear,
            tr("No gear worn yet. Bosses drop it for your class, for good."),
        );
        b.spawn(kit::text(&f.body, text, 14.0, color));
    });
    let class = selection.class_def();
    commands.entity(sub).despawn_related::<Children>();
    commands.entity(sub).with_children(|r| {
        if let Some(class) = class {
            r.spawn(kit::text(
                &f.body,
                tr(class.name),
                17.0,
                hud::rgb(class.color),
            ));
            r.spawn(kit::text(&f.body, tr("  ·  "), 17.0, theme::DIM));
        }
        r.spawn(kit::text(
            &f.body,
            format!(
                "{} of {total} pieces collected  ·  what you wear shows on the hero and adds \
                 to every run",
                collected.len()
            ),
            17.0,
            theme::TEXT,
        ));
    });
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|p| {
        kit::progress_bar(p, f, tr("Collection"), progress());
        p.spawn(kit::text(&f.body, summary.clone(), 12.0, theme::GREEN));
        p.spawn(kit::text(
            &f.body,
            tr("Each boss drops its own piece. Every piece found adds a quarter of its first bonus \
             to every run from then on, worn or not."),
            12.0,
            theme::DIM,
        ));
        for (z, map) in data::MAPS.iter().enumerate() {
            let table = data::zone_items(z);
            let found = table.iter().filter(|i| collected.contains(i)).count();
            p.spawn(Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(190.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.title, tr(map.name), 16.0, theme::GOLD));
                    c.spawn(kit::text(
                        &f.body,
                        trf(
                            "{} of {} found",
                            &[&found.to_string(), &table.len().to_string()],
                        ),
                        11.0,
                        theme::DIM,
                    ));
                });
                for item in &table {
                    let have = collected.contains(item);
                    let icon = if have {
                        displays
                            .as_deref()
                            .and_then(|d| outfit::gear_icon(item, class, &d.catalog))
                    } else {
                        None
                    }
                    .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".to_string());
                    let boss = data::roster(z).bosses[item.boss].name;
                    let tip = if have {
                        let mut lines = vec![
                            TipLine::new(item.name.clone(), hud::rgb(item.rarity.color())).header(),
                            TipLine::new(tr(item.slot.name()), Color::WHITE)
                                .right(tr(item.rarity.name()), theme::DIM),
                        ];
                        for b in &item.bonuses {
                            lines.push(TipLine::new(format!("+{}", bonus_text(b)), theme::GREEN));
                        }
                        lines.push(TipLine::new(
                            trf(
                                "Collected: +{} every run",
                                &[&bonus_text(&data::collection_bonus(item))],
                            ),
                            theme::GOLD,
                        ));
                        Tooltip(lines)
                    } else {
                        Tooltip(vec![
                            TipLine::new(tr("Not found yet"), Color::WHITE).header(),
                            TipLine::new(
                                format!("{} · dropped by {boss}", tr(item.slot.name())),
                                theme::DIM,
                            ),
                        ])
                    };
                    let e = kit::icon(row, IconSource::Texture(icon), 34.0, Some(tip));
                    if have {
                        row.commands()
                            .entity(e)
                            .insert(BorderColor::all(hud::rgb(item.rarity.color())));
                    } else {
                        row.commands()
                            .entity(e)
                            .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)));
                    }
                }
            });
        }
    });
}

/// The Talents tab: gold and points, then the class's three trees, every talent at its seat with
/// its rank; a click learns the next rank.
#[allow(clippy::too_many_arguments)]
fn refresh_talents(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    store: Res<ProfileStore>,
    talents: Option<Res<crate::ui_talent::Talents>>,
    spells: Option<Res<crate::ui_action::Spells>>,
    body: Query<Entity, With<TalentsBody>>,
    mut shown: Local<Option<String>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(body)) = (fonts, body.single()) else {
        return;
    };
    let class = selection.class_def();
    let look = menu::current_look(&selection);
    let p = &store.profile;
    let spent = class
        .and_then(|c| p.talents.get(c.name))
        .cloned()
        .unwrap_or_default();
    let view = format!(
        "{:?}{:?}{}{}{:?}{}{}",
        selection.class,
        look,
        p.gold,
        p.talent_points,
        spent,
        talents.is_some(),
        spells.is_some()
    );
    if shown.as_deref() == Some(view.as_str()) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let spells = spells.as_deref();
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|b| {
        let (Some(class), Some((race, _))) = (class, look) else {
            b.spawn(kit::text(&f.body, tr("Pick a class."), 13.0, theme::DIM));
            return;
        };
        let Some(catalog) = talents.as_deref().map(|t| &t.catalog) else {
            b.spawn(kit::text(
                &f.body,
                tr("The talent tables load with the game's data."),
                13.0,
                theme::DIM,
            ));
            return;
        };
        let trees = super::talents::trees(catalog, race, class.id);
        let used = super::talents::spent_total(&spent, &trees);
        let points = p.talent_points;
        b.spawn(Node {
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|r| {
            kit::button(
                r,
                f,
                tr("Back"),
                None,
                kit::ButtonLook::PLAIN,
                ScreenAction::Back,
            );
            kit::stat_tile(r, f, tr("Gold"), &p.gold.to_string());
            kit::stat_tile(
                r,
                f,
                tr("Points spent"),
                &format!("{used} of {points} (up to {})", super::talents::MAX_POINTS),
            );
            let cost = super::talents::point_cost(points);
            let full = points >= super::talents::MAX_POINTS;
            let train = kit::button(
                r,
                f,
                tr("Train a point"),
                Some(&if full {
                    tr("Every point is trained").to_string()
                } else {
                    trf("{} gold", &[&cost.to_string()])
                }),
                kit::ButtonLook::ACTION,
                ScreenAction::TrainPoint,
            );
            r.commands()
                .entity(train)
                .insert(kit::Disabled(full || p.gold < cost));
            let reset = kit::button(
                r,
                f,
                tr("Reset"),
                Some(class.name),
                kit::ButtonLook::PLAIN,
                ScreenAction::ResetTalents,
            );
            r.commands().entity(reset).insert((
                kit::Disabled(used == 0),
                Tooltip::simple(tr("Reset"), tr("Take back every point spent, for free.")),
            ));
        });
        b.spawn(kit::text(
            &f.body,
            tr(
                "Runs earn gold; gold trains points, shared by every class. Five points in a tree \
             open its next tier, as in WoW.",
            ),
            12.0,
            theme::DIM,
        ));
        b.spawn(Node {
            column_gap: Val::Px(14.0),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|cols| {
            for (ti, tree) in trees.iter().enumerate() {
                let in_tree = super::talents::spent_in(&spent, tree);
                kit::panel(
                    cols,
                    f,
                    Some(&format!("{} ({in_tree})", tree.tab.name)),
                    Node {
                        width: Val::Px(250.0),
                        ..default()
                    },
                    |p| {
                        let rows = tree.talents.iter().map(|t| t.row).max().unwrap_or(0);
                        kit::grid(p, 4, 6.0, |g| {
                            for row in 0..=rows {
                                for col in 0..4 {
                                    let Some((ki, t)) = tree
                                        .talents
                                        .iter()
                                        .enumerate()
                                        .find(|(_, t)| t.row == row && t.col == col)
                                    else {
                                        g.spawn(Node {
                                            width: Val::Px(36.0),
                                            height: Val::Px(36.0),
                                            ..default()
                                        });
                                        continue;
                                    };
                                    let r = super::talents::rank(&spent, t);
                                    let max = t.max_rank();
                                    let e = kit::icon_button(
                                        g,
                                        f,
                                        IconSource::Spell(t.ranks[0]),
                                        32.0,
                                        Some(&format!("{r}/{max}")),
                                        r > 0,
                                        ScreenAction::Learn(ti, ki),
                                    );
                                    let name = hud::spell_name(spells, t.ranks[0], "Talent");
                                    let per = super::talents::rank_bonus(tree.tab, t);
                                    let mut lines = vec![
                                        TipLine::new(name, Color::WHITE).header(),
                                        TipLine::new(
                                            trf("Rank {}/{}", &[&r.to_string(), &max.to_string()]),
                                            theme::DIM,
                                        ),
                                        TipLine::new(
                                            trf("+{} per rank", &[&bonus_text(&per)]),
                                            theme::GREEN,
                                        ),
                                    ];
                                    match super::talents::can_learn(&spent, &trees, tree, t, points)
                                    {
                                        Ok(()) => lines
                                            .push(TipLine::new(tr("Click to learn."), theme::GOLD)),
                                        Err(why) => lines.push(TipLine::new(why, theme::RED)),
                                    }
                                    let learnable =
                                        super::talents::can_learn(&spent, &trees, tree, t, points)
                                            .is_ok();
                                    g.commands().entity(e).insert((
                                        Tooltip(lines),
                                        kit::Disabled(r == 0 && !learnable),
                                    ));
                                }
                            }
                        });
                    },
                );
            }
        });
    });
}

/// "1 in 1,500": the odds of one kill, as a reader takes them.
fn one_in(chance: f32) -> String {
    if chance <= 0.0 {
        return "never".into();
    }
    let n = (1.0 / chance).round() as u64;
    let digits = n.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    trf("1 in {}", &[&grouped])
}

/// The Drop Tables tab: every zone's five pieces, who drops each and at what odds.
#[allow(clippy::too_many_arguments)]
fn refresh_drops(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    selection: Res<Selection>,
    store: Res<ProfileStore>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    body: Query<Entity, With<DropsBody>>,
    mut shown: Local<Option<(usize, usize, Option<usize>, bool)>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(body)) = (fonts, body.single()) else {
        return;
    };
    let collected = store.profile.collected();
    let view = (
        collected.len(),
        open_zones(&store),
        selection.class,
        displays.is_some(),
    );
    if *shown == Some(view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let class = selection.class_def();
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|p| {
        p.spawn(kit::text(
            &f.body,
            format!(
                "A boss always drops its own piece. Any other kill drops one of its zone's \
                 pieces {} times in all, the rare half half as often.",
                one_in(1.0 / data::HORDE_ITEM_ODDS as f32)
            ),
            12.0,
            theme::DIM,
        ));
        for (z, map) in data::MAPS.iter().enumerate() {
            let open = store.profile.zone_open(z);
            kit::heading(p, f, tr(map.name));
            if !open {
                if let Some(hint) = data::unlock_hint(z) {
                    p.spawn(kit::text(
                        &f.body,
                        trf("Locked · {}", &[&hint]),
                        11.0,
                        theme::RED,
                    ));
                }
            }
            for item in data::zone_items(z) {
                let have = collected.contains(&item);
                let boss = data::roster(z).bosses[item.boss].name;
                p.spawn(Node {
                    column_gap: Val::Px(10.0),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    let icon = displays
                        .as_deref()
                        .and_then(|d| outfit::gear_icon(item, class, &d.catalog))
                        .unwrap_or_else(|| "Interface\\Icons\\INV_Misc_QuestionMark".to_string());
                    let e = kit::icon(row, IconSource::Texture(icon), 28.0, None);
                    if !have {
                        row.commands()
                            .entity(e)
                            .insert(BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)));
                    }
                    row.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        width: Val::Px(280.0),
                        ..default()
                    })
                    .with_children(|c| {
                        c.spawn(kit::text(
                            &f.body,
                            item.name.clone(),
                            13.0,
                            hud::rgb(item.rarity.color()),
                        ));
                        let bonuses: Vec<String> = item
                            .bonuses
                            .iter()
                            .map(|b| format!("+{}", bonus_text(b)))
                            .collect();
                        c.spawn(kit::text(
                            &f.body,
                            format!("{} · {}", tr(item.slot.name()), bonuses.join(" · ")),
                            11.0,
                            theme::DIM,
                        ));
                    });
                    kit::chip(row, f, &trf("{}: always", &[boss]), true);
                    kit::chip(
                        row,
                        f,
                        &trf("Horde: {}", &[&one_in(data::horde_chance(item))]),
                        false,
                    );
                    if have {
                        row.spawn(kit::text(&f.body, tr("Found"), 11.0, theme::GREEN));
                    }
                });
            }
        }
    });
}

/// The Codex: the horde, the bosses and the spells met in the runs, and the pieces found.
#[allow(clippy::too_many_arguments)]
fn refresh_codex(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    store: Res<ProfileStore>,
    spells: Option<Res<crate::ui_action::Spells>>,
    body: Query<Entity, With<CodexBody>>,
    mut shown: Local<Option<(usize, usize, usize, usize, bool)>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(body)) = (fonts, body.single()) else {
        return;
    };
    let codex = &store.profile.codex;
    let view = (
        codex.horde.len(),
        codex.bosses.len(),
        codex.spells.len(),
        store.profile.collected().len(),
        spells.is_some(),
    );
    if *shown == Some(view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let spells = spells.as_deref();
    let (horde, bosses) = data::all_creatures();
    let stats = super::Stats::base();
    let unseen = theme::DIM.with_alpha(0.6);
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|p| {
        let seen_of = |all: &[&str], set: &std::collections::BTreeSet<String>| {
            all.iter().filter(|n| set.contains(**n)).count()
        };
        let spell_keys: Vec<&str> = data::ABILITIES
            .iter()
            .map(|a| a.key)
            .chain(data::PASSIVES.iter().map(|p| p.key))
            .collect();
        p.spawn(Node {
            column_gap: Val::Px(12.0),
            ..default()
        })
        .with_children(|r| {
            for (label, value, max) in [
                ("Horde", seen_of(&horde, &codex.horde), horde.len()),
                ("Bosses", seen_of(&bosses, &codex.bosses), bosses.len()),
                (
                    "Spells",
                    seen_of(&spell_keys, &codex.spells),
                    spell_keys.len(),
                ),
                ("Items", store.profile.collected().len(), data::ITEMS.len()),
            ] {
                r.spawn(Node {
                    width: Val::Px(200.0),
                    ..default()
                })
                .with_children(|c| {
                    kit::progress_bar(
                        c,
                        f,
                        tr(label),
                        kit::Progress {
                            value: value as f32,
                            max: max as f32,
                        },
                    );
                });
            }
        });
        kit::heading(p, f, tr("Bosses"));
        for (z, map) in data::MAPS.iter().enumerate() {
            p.spawn(Node {
                column_gap: Val::Px(6.0),
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    width: Val::Px(170.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.title, tr(map.name), 14.0, theme::GOLD));
                });
                for (t, name) in boss_timeline(z) {
                    let met = codex.bosses.contains(name);
                    let label = if met {
                        format!("{} · {name}", mmss(t))
                    } else {
                        format!("{} · ???", mmss(t))
                    };
                    kit::chip(row, f, &label, met);
                }
            });
        }
        kit::heading(p, f, tr("Horde"));
        for (z, map) in data::MAPS.iter().enumerate() {
            let names = enemy_names(z);
            let met: Vec<String> = names
                .iter()
                .map(|n| {
                    if codex.horde.contains(*n) {
                        n.to_string()
                    } else {
                        "???".to_string()
                    }
                })
                .collect();
            let count = names.iter().filter(|n| codex.horde.contains(**n)).count();
            p.spawn(Node {
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    width: Val::Px(170.0),
                    flex_shrink: 0.0,
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(
                        &f.title,
                        format!("{} ({count}/{})", map.name, names.len()),
                        14.0,
                        theme::GOLD,
                    ));
                });
                row.spawn(kit::text(&f.body, met.join(", "), 11.0, theme::TEXT));
            });
        }
        kit::heading(p, f, tr("Spells"));
        p.spawn(Node {
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            flex_wrap: FlexWrap::Wrap,
            ..default()
        })
        .with_children(|g| {
            for a in data::ABILITIES {
                let met = codex.spells.contains(a.key);
                let tip = if met {
                    Tooltip(spell_tip(a, &stats, spells))
                } else {
                    Tooltip::simple(tr("Not played yet"), tr("Take it from a level-up card."))
                };
                let e = kit::icon(g, IconSource::Spell(a.spell), 30.0, Some(tip));
                if !met {
                    g.commands().entity(e).insert(BackgroundColor(unseen));
                }
            }
            for ps in data::PASSIVES {
                let met = codex.spells.contains(ps.key);
                let tip = if met {
                    Tooltip::simple(ps.name, format!("+{} per rank", bonus_text(&ps.bonus(1))))
                } else {
                    Tooltip::simple(tr("Not played yet"), tr("Take it from a level-up card."))
                };
                let e = kit::icon(g, IconSource::Spell(ps.spell), 30.0, Some(tip));
                if !met {
                    g.commands().entity(e).insert(BackgroundColor(unseen));
                }
            }
        });
    });
}

/// The Achievements tab: those done and waiting to be claimed, then every one with its progress;
/// the Achievements tab's count of claims waiting, and the Rewards tab's of points to spend.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn refresh_achievements(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    store: Res<ProfileStore>,
    bodies: Query<Entity, With<AchievementsBody>>,
    tabs: Query<(&TabButton, &Children)>,
    mut texts: Query<&mut Text>,
    mut shown: Local<Option<String>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let Some(fonts) = fonts else { return };
    let p = &store.profile;
    let view = format!(
        "{:?}{:?}{:?}{}{}{}{}{}",
        p.totals,
        p.claimed,
        p.perks,
        p.gold,
        p.talent_points,
        p.inventory.len(),
        p.codex.bosses.len(),
        p.zones.len()
    );
    if shown.as_deref() == Some(view.as_str()) {
        return;
    }
    let Ok(achievements) = bodies.single() else {
        return;
    };
    *shown = Some(view);
    let unclaimed = p.unclaimed();
    // The Achievements tab says how many claims wait; the Rewards tab, how many points.
    let points = p.points_left();
    for (tab, children) in &tabs {
        let want = match tab.0 {
            ACHIEVEMENTS if !unclaimed.is_empty() => {
                trf("Achievements ({})", &[&unclaimed.len().to_string()])
            }
            ACHIEVEMENTS => tr(TABS[ACHIEVEMENTS]).to_string(),
            REWARDS if points > 0 => trf("Rewards ({})", &[&points.to_string()]),
            REWARDS => tr(TABS[REWARDS]).to_string(),
            _ => continue,
        };
        for child in children.iter() {
            if let Ok(mut t) = texts.get_mut(child) {
                if **t != want {
                    **t = want.clone();
                }
            }
        }
    }
    let f = &*fonts;
    let done = data::ACHIEVEMENTS.iter().filter(|x| p.achieved(x)).count();
    commands.entity(achievements).despawn_related::<Children>();
    commands.entity(achievements).with_children(|body| {
        if !unclaimed.is_empty() {
            let total: u64 = unclaimed.iter().map(|x| x.gold).sum();
            kit::button(
                body,
                f,
                tr("Claim all"),
                Some(&trf("{} gold", &[&total.to_string()])),
                kit::ButtonLook::ACTION,
                ScreenAction::ClaimAll,
            );
        }
        kit::progress_bar(
            body,
            f,
            tr("Achievements"),
            kit::Progress {
                value: done as f32,
                max: data::ACHIEVEMENTS.len() as f32,
            },
        );
        for (i, x) in data::ACHIEVEMENTS.iter().enumerate() {
            let (now, goal) = p.progress(x);
            let ok = now >= goal;
            let claimed = p.claimed.contains(&x.key);
            body.spawn(Node {
                column_gap: Val::Px(12.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    width: Val::Px(320.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(
                        &f.title,
                        x.name.clone(),
                        15.0,
                        if ok { theme::GOLD } else { theme::TEXT },
                    ));
                    c.spawn(kit::text(&f.body, x.text.clone(), 11.0, theme::DIM));
                });
                row.spawn(Node {
                    width: Val::Px(220.0),
                    ..default()
                })
                .with_children(|c| {
                    kit::progress_bar(
                        c,
                        f,
                        "",
                        kit::Progress {
                            value: now.min(goal) as f32,
                            max: goal as f32,
                        },
                    );
                });
                kit::chip(
                    row,
                    f,
                    &trf("{} gold", &[&x.gold.to_string()]),
                    ok && !claimed,
                );
                // Its reward points come with it done, for the Rewards tab.
                kit::chip(row, f, &trf("{} points", &[&x.points().to_string()]), ok);
                if ok && !claimed {
                    kit::button(
                        row,
                        f,
                        tr("Claim"),
                        None,
                        kit::ButtonLook::ACTION,
                        ScreenAction::Claim(i),
                    );
                } else if claimed {
                    row.spawn(kit::text(&f.body, tr("Claimed"), 11.0, theme::GREEN));
                }
            });
        }
    });
}

/// A new language rebuilds the main screen, every label in it.
fn rebuild_on_language(
    mut commands: Commands,
    roots: Query<Entity, With<ScreenRoot>>,
    mut page: ResMut<Page>,
    mut seen: Local<u32>,
) {
    if !lang::stale(&mut seen) {
        return;
    }
    for e in &roots {
        commands.entity(e).despawn();
    }
    // The pages show again once built.
    page.set_changed();
}

/// The Options page: sound, mouse, picture, language, and the controls.
fn refresh_options(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    store: Res<ProfileStore>,
    body: Query<Entity, With<OptionsBody>>,
    mut shown: Local<Option<super::profile::Options>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(body)) = (fonts, body.single()) else {
        return;
    };
    let o = store.profile.options.clamped();
    if shown.as_ref() == Some(&o) {
        return;
    }
    *shown = Some(o.clone());
    let f = &*fonts;
    let percent = |v: f32| format!("{:.0}%", v * 100.0);
    commands.entity(body).despawn_related::<Children>();
    commands.entity(body).with_children(|p| {
        kit::button(
            p,
            f,
            tr("Back"),
            None,
            kit::ButtonLook::PLAIN,
            ScreenAction::Back,
        );
        let rows: [(&str, String, Setting); 6] = [
            ("Master volume", percent(o.master), Setting::Master),
            ("Music", percent(o.music), Setting::Music),
            ("Sound effects", percent(o.sfx), Setting::Sfx),
            (
                "Mouse sensitivity",
                format!("×{:.1}", o.mouse_sens),
                Setting::Sens,
            ),
            (
                "Picture polish",
                tr(if o.polish { "On" } else { "Off" }).to_string(),
                Setting::Polish,
            ),
            (
                "Screen shake",
                tr(if o.shake { "On" } else { "Off" }).to_string(),
                Setting::Shake,
            ),
        ];
        for (label, value, setting) in rows {
            p.spawn(Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    width: Val::Px(220.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.body, tr(label), 14.0, theme::TEXT));
                });
                if matches!(setting, Setting::Polish | Setting::Shake) {
                    kit::button(
                        row,
                        f,
                        &value,
                        None,
                        kit::ButtonLook::PLAIN,
                        ScreenAction::Set(setting, 1),
                    );
                    return;
                }
                kit::button(
                    row,
                    f,
                    tr("−"),
                    None,
                    kit::ButtonLook::PLAIN,
                    ScreenAction::Set(setting, -1),
                );
                row.spawn(Node {
                    width: Val::Px(60.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.body, value.clone(), 14.0, theme::GOLD));
                });
                kit::button(
                    row,
                    f,
                    tr("+"),
                    None,
                    kit::ButtonLook::PLAIN,
                    ScreenAction::Set(setting, 1),
                );
            });
        }
        p.spawn(Node {
            column_gap: Val::Px(10.0),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|row| {
            row.spawn(Node {
                width: Val::Px(220.0),
                ..default()
            })
            .with_children(|c| {
                c.spawn(kit::text(&f.body, tr("Language"), 14.0, theme::TEXT));
            });
            for (i, l) in lang::Lang::ALL.iter().enumerate() {
                let b = kit::button(
                    row,
                    f,
                    l.name(),
                    None,
                    kit::ButtonLook::PLAIN,
                    ScreenAction::Language(i),
                );
                row.commands()
                    .entity(b)
                    .insert(kit::Selected(l.code() == o.language));
            }
        });
        kit::heading(p, f, tr("Controls"));
        for (keys, what) in [
            ("W A S D", "Run where the camera looks"),
            (tr("Both mouse buttons"), "Run forward"),
            ("Mouse", "Look around"),
            ("Space", "Jump"),
            ("F", "Movement spell"),
            ("C", "Switch the camera"),
            ("G", "Toggle the picture polish"),
            ("1 2 3", "Pick a card"),
            ("R", "Reroll the cards"),
            ("Esc", "Pause"),
        ] {
            p.spawn(Node {
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                row.spawn(Node {
                    width: Val::Px(220.0),
                    ..default()
                })
                .with_children(|c| {
                    c.spawn(kit::text(&f.body, keys, 13.0, theme::GOLD));
                });
                row.spawn(kit::text(&f.body, tr(what), 13.0, theme::TEXT));
            });
        }
    });
}

/// Minutes and seconds, "2:15".
fn mmss(secs: f32) -> String {
    let s = secs.max(0.0).round() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// A zone's bosses and when each comes in a first stage, under the solo rules: `(time, name)`, the
/// final boss last, at the time its portal opens.
fn boss_timeline(zone: usize) -> Vec<(f32, &'static str)> {
    let rules = super::Run::default().threat(data::Rules::SOLO);
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
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
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
                    tr(map.name),
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
                    b.spawn(kit::text(
                        &f.body,
                        trf("Locked · {}", &[&hint]),
                        11.0,
                        theme::RED,
                    ));
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
                        trf("Defeat {} to win.", &[final_boss]),
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
    mut shown: Local<Option<(usize, bool, bool, usize, usize)>>,
    mut seen_language: Local<u32>,
) {
    if rebuilt(&mut seen_language) {
        *shown = None;
    }
    let (Some(fonts), Ok(bar)) = (fonts, bar.single()) else {
        return;
    };
    let open = store.profile.zone_open(selection.map);
    let hardest = store.profile.hardest_open(selection.map_def().name);
    let difficulty = selection.difficulty.min(hardest);
    let view = (selection.map, session.in_lobby, open, difficulty, hardest);
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
        kit::button(
            r,
            f,
            tr("Options"),
            None,
            kit::ButtonLook::PLAIN,
            ScreenAction::Options,
        );
        let d = data::DIFFICULTIES[difficulty];
        let options = kit::button(
            r,
            f,
            tr("Run Options"),
            Some(&trf("{} difficulty", &[tr(d.name)])),
            kit::ButtonLook::PLAIN,
            ScreenAction::RunOptions,
        );
        let mut lines = vec![TipLine::new(tr("Difficulty"), Color::WHITE).header()];
        for (i, x) in data::DIFFICULTIES.iter().enumerate() {
            let (text, color) = if i <= hardest {
                (
                    format!(
                        "{}: enemies ×{:.1} health, ×{:.1} damage, ×{:.2} gold",
                        x.name, x.health, x.damage, x.reward
                    ),
                    if i == difficulty {
                        theme::GOLD
                    } else {
                        theme::TEXT
                    },
                )
            } else {
                (
                    format!(
                        "{}: win {} on {} to open it",
                        x.name,
                        map.name,
                        data::DIFFICULTIES[i - 1].name
                    ),
                    theme::DIM,
                )
            };
            lines.push(TipLine::new(text, color));
        }
        lines.push(TipLine::new(
            if session.in_lobby {
                "A lobby plays by its host's rules."
            } else {
                "Click for the next one."
            },
            theme::DIM,
        ));
        r.commands().entity(options).insert((
            kit::Disabled(session.in_lobby || hardest == 0),
            Tooltip(lines),
        ));
        let sub = if open || session.in_lobby {
            trf(
                "Defeat {} at {} · {} bosses on the way",
                &[
                    final_boss,
                    &mmss(data::Rules::SOLO.duration),
                    &timeline.len().to_string(),
                ],
            )
        } else {
            data::unlock_hint(selection.map).unwrap_or_default()
        };
        let enter = kit::big_button(
            r,
            f,
            &trf("Enter {}", &[tr(map.name)]),
            &sub,
            ScreenAction::Enter,
        );
        if !open && !session.in_lobby {
            r.commands().entity(enter).insert(kit::Disabled(true));
        }
        let (title, sub) = if session.in_lobby {
            (tr("Co-op"), tr("Back to your lobby"))
        } else {
            (tr("Co-op"), tr("Play with friends"))
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
            assert_eq!(mmss(t.last().unwrap().0), "9:36");
            assert!(!enemy_names(i).is_empty());
        }
        let elwynn: Vec<String> = boss_timeline(0)
            .iter()
            .map(|(t, n)| format!("{} {n}", mmss(*t)))
            .collect();
        assert_eq!(
            elwynn,
            [
                "1:48 Hogger",
                "3:36 Mother Fang",
                "5:24 Rhahk'Zor",
                "7:12 Mr. Smite",
                "9:36 Edwin VanCleef",
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
    fn the_collection_line_adds_up() {
        assert!(bonus_summary(&[]).starts_with("Find gear"));
        let line = bonus_summary(&[
            data::Bonus::flat(data::Stat::MaxHp, 10.0),
            data::Bonus::flat(data::Stat::MaxHp, 15.0),
            data::Bonus::flat(data::Stat::Crit, 0.01),
        ]);
        assert_eq!(line, "+25 max health · +1% critical strike");
        assert_eq!(
            bonus_text(&data::Bonus::pct(data::Stat::Magnet, 0.08)),
            "8% pickup range"
        );
        assert_eq!(
            bonus_text(&data::Bonus::flat(data::Stat::Armor, 1.5)),
            "1.5 damage blocked per hit"
        );
        assert!(BUILT_TABS.iter().all(|&t| t < TABS.len()));
        assert_eq!(TABS[TALENTS], "Talents");
        assert_eq!(TABS[DROPS], "Drop Tables");
        assert_eq!(TABS[CODEX], "Codex");
        assert_eq!(TABS[ACHIEVEMENTS], "Achievements");
        assert_eq!(TABS[REWARDS], "Rewards");
        assert_eq!(BUILT_TABS.len(), TABS.len(), "every tab is built");
        assert_eq!(one_in(1.0 / 600.0), "1 in 600");
        assert_eq!(one_in(1.0 / 1500.0), "1 in 1,500");
        assert_eq!(one_in(0.0), "never");
        assert!(!BUILT_TABS.contains(&ARMORY));
    }

    #[test]
    fn the_tabs_match_the_screen() {
        assert_eq!(TABS[0], "Character");
        assert_eq!(TABS.len(), 7);
    }
}
