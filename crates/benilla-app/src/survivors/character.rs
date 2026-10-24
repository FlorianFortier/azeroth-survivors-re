//! The main screen: the title, the tabs, and the Character tab's three columns — the class on
//! the left, the hero in the middle, the zone on the right — over the slowly orbiting hero.
//!
//! It is built from [`super::kit`] and drives the same [`Selection`] and profile as the glue
//! menu ([`super::menu`]). Until every column is in, it replaces that menu only under
//! `WOW_NEW_MENU=1` (`?new_menu=1` on the web).
//!
//! Today: the class column (the class grid, the class's description, its starting health with
//! everything the profile brings in, its starting spell, the zones open, and its spell pool with
//! tooltips). The hero card and the zone list come next.

use bevy::prelude::*;

use super::data::{self, ClassDef};
use super::hud::{self, TipLine};
use super::kit::{self, theme, IconSource, KitClicks, KitFonts, Tooltip};
use super::profile::ProfileStore;
use super::{menu, Owned, Phase, Run, Selection, Vitals};

/// The new screen replaces the glue menu.
pub(super) fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| crate::webenv::var("WOW_NEW_MENU").is_some_and(|v| v != "0"))
}

pub(super) fn plugin(app: &mut App) {
    if !enabled() {
        return;
    }
    app.add_systems(
        Update,
        (build_screen, show_screen, clicks, refresh_class_column)
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
    Enter,
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
                cols.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                kit::panel(
                    cols,
                    f,
                    Some("Zone"),
                    Node {
                        width: Val::Px(300.0),
                        ..default()
                    },
                    |p| {
                        p.spawn(kit::text(
                            &f.body,
                            "The zone list comes with the next update; the zone last picked is used.",
                            12.0,
                            theme::DIM,
                        ));
                    },
                );
            });
            // The call to action.
            root.spawn(Node {
                column_gap: Val::Px(12.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                kit::big_button(r, f, "Enter World", "Start the run", ScreenAction::Enter);
            });
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
) {
    if *phase.get() != Phase::Menu {
        return;
    }
    let Some(&action) = clicks.first(&buttons) else {
        return;
    };
    match action {
        ScreenAction::Tab(_) => {}
        ScreenAction::Class(c) => {
            let Some(creatures) = creatures.as_deref() else {
                return;
            };
            let all = menu::all_looks(creatures);
            menu::pick_class(&mut selection, &all, c);
            menu::save_pick(&selection, &mut store);
        }
        ScreenAction::Enter => {
            if session.in_lobby {
                next.set(Phase::Lobby);
            } else if hud::start_run(&mut run, &mut selection) {
                next.set(Phase::Playing);
            }
        }
    }
}

/// What the class column shows, to rebuild it only when that changes.
#[derive(PartialEq, Clone, Debug)]
struct ClassView {
    class: Option<usize>,
    race: u8,
    spells_ready: bool,
    bonuses: usize,
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
    let (meta, procs) = store.profile.run_bonuses();
    let view = ClassView {
        class: selection.class,
        race,
        spells_ready: spells.is_some(),
        bonuses: meta.len() + procs.len(),
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
            let zones = data::MAPS.len();
            kit::stat_tile(r, f, "Zones open", &format!("{zones} of {zones}"));
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
    fn the_tabs_match_the_screen() {
        assert_eq!(TABS[0], "Character");
        assert_eq!(TABS.len(), 7);
    }
}
