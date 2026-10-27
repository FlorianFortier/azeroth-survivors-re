//! The Rewards tab: reward points, earned by achievements ([`data::AchievementDef::points`]) and
//! spent on what every hero of every class brings into a run. The perks ([`data::PERKS`]): more
//! rerolls and banishes, more gold for a skipped hand, a fuller first hand, more experience and
//! gold, gold in hand at the start. The races: one opened ([`data::RACE_POINTS`]) can be every
//! class, its look added to each class's ([`super::menu::open_looks`]). The tabards
//! ([`data::TABARDS`]), 1.12.1's own, worn over the gear: some bought, some earned by an
//! achievement, one worn at a time. The appearances ([`data::APPEARANCES`]): 1.12.1's hats,
//! helms and cloaks worn over the gear's look on the head and back, or the slot shown bare;
//! earned the same ways. All are bought on a draft and kept with Confirm (Undo drops the draft);
//! Reset takes every perk point back, [`data::MAX_PERK_RESETS`] times a save, and leaves the
//! rest.
//!
//! A perk's bonus rides the profile's other bonuses (`Profile::meta_bonuses`, so a guest's
//! loadout carries it); the rest lands on the hero when its run begins ([`apply_perks`]), on the
//! host for a guest from the ranks its loadout carried ([`seat_perks`]).

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;

use super::data::{self, PerkDef, PerkEffect};
use super::hud::TipLine;
use super::kit::{self, theme, IconSource, KitClicks, KitFonts, Tooltip};
use super::lang::{tr, trf};
use super::profile::{Profile, ProfileStore, Spend};
use super::Vitals;

/// What a perk's ranks do that is not a bonus: rerolls and banishes in hand, a skip's gold, the
/// first hand's extra cards, gold at the start. The bonuses go with the profile's
/// (`Profile::meta_bonuses`).
pub(super) fn apply_perks(v: &mut Vitals, ranks: impl IntoIterator<Item = (&'static PerkDef, u8)>) {
    for (perk, rank) in ranks {
        let rank = rank.min(perk.max_rank);
        match perk.effect {
            PerkEffect::Rerolls(n) => v.rerolls = v.rerolls.saturating_add(n * rank),
            PerkEffect::Banishes(n) => v.banishes = v.banishes.saturating_add(n * rank),
            PerkEffect::SkipGold(n) => v.skip_gold += n * u32::from(rank),
            PerkEffect::OpeningHand(n) => v.opening_hand = v.opening_hand.saturating_add(n * rank),
            PerkEffect::StartGold(n) => v.gold += (n * u32::from(rank)) as f32,
            PerkEffect::Bonus(_) => {}
        }
    }
}

/// A guest's perks as its loadout carried them: the ones the tables know, each held to its most.
pub(super) fn seat_perks(seat: &super::Seat) -> Vec<(&'static PerkDef, u8)> {
    seat.brings
        .perks
        .iter()
        .filter_map(|(k, r)| data::perk(k).map(|p| (p, (*r).min(p.max_rank))))
        .collect()
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<Draft>()
        .add_systems(Update, (reward_clicks, refresh_rewards).chain());
}

/// The Rewards tab's sections; the perks and the races are built so far.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Section {
    #[default]
    Perks,
    Companions,
    Tabards,
    Races,
    Appearances,
}

const SECTIONS: [(Section, &str); 5] = [
    (Section::Perks, "Perks"),
    (Section::Companions, "Companions"),
    (Section::Tabards, "Tabards"),
    (Section::Races, "Races"),
    (Section::Appearances, "Appearances"),
];

/// What is being bought, before Confirm keeps it; each `None` while it matches the profile.
#[derive(Resource, Default)]
struct Draft {
    perks: Option<BTreeMap<String, u8>>,
    races: Option<BTreeSet<String>>,
    tabards: Option<BTreeSet<String>>,
    appearances: Option<BTreeSet<String>>,
    section: Section,
}

impl Draft {
    fn perks<'a>(&'a self, profile: &'a Profile) -> &'a BTreeMap<String, u8> {
        self.perks.as_ref().unwrap_or(&profile.perks)
    }

    fn races<'a>(&'a self, profile: &'a Profile) -> &'a BTreeSet<String> {
        self.races.as_ref().unwrap_or(&profile.races)
    }

    fn tabards<'a>(&'a self, profile: &'a Profile) -> &'a BTreeSet<String> {
        self.tabards.as_ref().unwrap_or(&profile.tabards)
    }

    fn appearances<'a>(&'a self, profile: &'a Profile) -> &'a BTreeSet<String> {
        self.appearances.as_ref().unwrap_or(&profile.appearances)
    }

    /// Everything the draft holds.
    fn spend<'a>(&'a self, profile: &'a Profile) -> Spend<'a> {
        Spend {
            perks: self.perks(profile),
            races: self.races(profile),
            tabards: self.tabards(profile),
            appearances: self.appearances(profile),
        }
    }

    fn dirty(&self) -> bool {
        self.perks.is_some()
            || self.races.is_some()
            || self.tabards.is_some()
            || self.appearances.is_some()
    }

    fn clear(&mut self) {
        *self = Self {
            section: self.section,
            ..Self::default()
        };
    }
}

#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
enum RewardAction {
    Section(Section),
    Up(usize),
    Down(usize),
    /// Open a race on the draft, or take it off again.
    Race(u8),
    /// A tabard owned: wear it, or take it off; one for points: put it on the draft, or take it
    /// off again.
    Tabard(usize),
    /// The same for an appearance, on its slot.
    Appearance(usize),
    Confirm,
    Undo,
    Reset,
}

/// The page's two panels, filled by [`refresh_rewards`].
#[derive(Component)]
struct RewardsSide;
#[derive(Component)]
struct RewardsMain;

/// The Rewards page, built by the main screen in its tab's place.
pub(super) fn page(p: &mut ChildSpawnerCommands, f: &KitFonts) {
    p.spawn(Node {
        width: Val::Percent(100.0),
        flex_grow: 1.0,
        min_height: Val::Px(0.0),
        column_gap: Val::Px(22.0),
        ..default()
    })
    .with_children(|cols| {
        kit::panel(
            cols,
            f,
            Some(tr("Rewards")),
            Node {
                width: Val::Px(360.0),
                flex_shrink: 0.0,
                ..default()
            },
            |p| {
                p.spawn((
                    RewardsSide,
                    Node {
                        flex_direction: FlexDirection::Column,
                        flex_grow: 1.0,
                        row_gap: Val::Px(8.0),
                        ..default()
                    },
                ));
            },
        );
        kit::panel(
            cols,
            f,
            None,
            Node {
                flex_grow: 1.0,
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
                    |body| {
                        body.spawn((
                            RewardsMain,
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(10.0),
                                ..default()
                            },
                        ));
                    },
                );
            },
        );
    });
}

fn reward_clicks(
    clicks: Res<KitClicks>,
    buttons: Query<(Entity, &RewardAction)>,
    mut draft: ResMut<Draft>,
    mut store: ResMut<ProfileStore>,
) {
    let Some(&action) = clicks.first(&buttons) else {
        return;
    };
    match action {
        RewardAction::Section(s) => draft.section = s,
        RewardAction::Up(i) | RewardAction::Down(i) => {
            let Some(perk) = data::PERKS.get(i) else {
                return;
            };
            let mut perks = draft.perks(&store.profile).clone();
            let rank = perks.get(perk.key).copied().unwrap_or(0);
            let want = if matches!(action, RewardAction::Up(_)) {
                rank.saturating_add(1).min(perk.max_rank)
            } else {
                rank.saturating_sub(1)
            };
            perks.insert(perk.key.to_string(), want);
            perks.retain(|_, r| *r > 0);
            let p = &store.profile;
            let cost = Spend {
                perks: &perks,
                ..draft.spend(p)
            }
            .cost();
            if cost <= p.points_earned() {
                draft.perks = (perks != p.perks).then_some(perks);
            }
        }
        RewardAction::Race(race) => {
            if store.profile.race_open(race) {
                return;
            }
            let name = crate::char_select::race_name(race).to_string();
            let mut races = draft.races(&store.profile).clone();
            if !races.remove(&name) {
                races.insert(name);
            }
            let p = &store.profile;
            let cost = Spend {
                races: &races,
                ..draft.spend(p)
            }
            .cost();
            if cost <= p.points_earned() {
                draft.races = (races != p.races).then_some(races);
            }
        }
        RewardAction::Tabard(i) => {
            let Some(t) = data::TABARDS.get(i) else {
                return;
            };
            if store.profile.tabard_owned(t) {
                let worn = store.profile.tabard == t.key;
                store.edit(|p| {
                    p.wear_tabard((!worn).then_some(t.key));
                });
                return;
            }
            if !matches!(t.unlock, data::Unlock::Points(_)) {
                return;
            }
            let mut tabards = draft.tabards(&store.profile).clone();
            toggle(&mut tabards, t.key);
            let p = &store.profile;
            let cost = Spend {
                tabards: &tabards,
                ..draft.spend(p)
            }
            .cost();
            if cost <= p.points_earned() {
                draft.tabards = (tabards != p.tabards).then_some(tabards);
            }
        }
        RewardAction::Appearance(i) => {
            let Some(a) = data::APPEARANCES.get(i) else {
                return;
            };
            if store.profile.appearance_owned(a) {
                let worn =
                    store.profile.looks.get(a.slot.name()).map(String::as_str) == Some(a.key);
                store.edit(|p| {
                    p.wear_appearance(a.slot, (!worn).then_some(a.key));
                });
                return;
            }
            if !matches!(a.unlock, data::Unlock::Points(_)) {
                return;
            }
            let mut appearances = draft.appearances(&store.profile).clone();
            toggle(&mut appearances, a.key);
            let p = &store.profile;
            let cost = Spend {
                appearances: &appearances,
                ..draft.spend(p)
            }
            .cost();
            if cost <= p.points_earned() {
                draft.appearances = (appearances != p.appearances).then_some(appearances);
            }
        }
        RewardAction::Confirm => {
            // Perks first: the races and tabards already bought count against them, and the new
            // ones against the perks just kept, so a draft that fits in all goes through.
            let (perks, races, tabards, appearances) = (
                draft.perks.take(),
                draft.races.take(),
                draft.tabards.take(),
                draft.appearances.take(),
            );
            store.edit(|p| {
                if let Some(perks) = perks {
                    p.set_perks(perks);
                }
                if let Some(races) = races {
                    p.open_races(races);
                }
                if let Some(tabards) = tabards {
                    p.buy_tabards(tabards);
                }
                if let Some(appearances) = appearances {
                    p.buy_appearances(appearances);
                }
            });
        }
        RewardAction::Undo => draft.clear(),
        RewardAction::Reset => {
            draft.clear();
            store.edit(|p| {
                p.reset_perks();
            });
        }
    }
}

/// Put `key` on a draft's set, or take it off again.
fn toggle(set: &mut BTreeSet<String>, key: &str) {
    if !set.remove(key) {
        set.insert(key.to_string());
    }
}

/// What the page shows, to rebuild it only when that changes.
#[derive(PartialEq, Clone, Debug)]
struct View {
    side: Entity,
    main: Entity,
    earned: u32,
    profile_perks: BTreeMap<String, u8>,
    draft: Option<BTreeMap<String, u8>>,
    profile_races: BTreeSet<String>,
    draft_races: Option<BTreeSet<String>>,
    profile_tabards: BTreeSet<String>,
    draft_tabards: Option<BTreeSet<String>>,
    tabard: String,
    /// The tabards owned, achievements' included.
    owned: Vec<bool>,
    profile_appearances: BTreeSet<String>,
    draft_appearances: Option<BTreeSet<String>>,
    looks: BTreeMap<String, String>,
    /// The appearances owned, achievements' included.
    owned_looks: Vec<bool>,
    icons: bool,
    resets: u8,
    section: Section,
    lang: u32,
}

fn refresh_rewards(
    mut commands: Commands,
    fonts: Option<Res<KitFonts>>,
    store: Res<ProfileStore>,
    draft: Res<Draft>,
    displays: Option<Res<crate::entities::ItemDisplays>>,
    side: Query<Entity, With<RewardsSide>>,
    main: Query<Entity, With<RewardsMain>>,
    mut shown: Local<Option<View>>,
    mut lang_seen: Local<u32>,
) {
    let (Some(fonts), Ok(side), Ok(main)) = (fonts, side.single(), main.single()) else {
        return;
    };
    super::lang::stale(&mut lang_seen);
    let p = &store.profile;
    let view = View {
        side,
        main,
        earned: p.points_earned(),
        profile_perks: p.perks.clone(),
        draft: draft.perks.clone(),
        profile_races: p.races.clone(),
        draft_races: draft.races.clone(),
        profile_tabards: p.tabards.clone(),
        draft_tabards: draft.tabards.clone(),
        tabard: p.tabard.clone(),
        owned: data::TABARDS.iter().map(|t| p.tabard_owned(t)).collect(),
        profile_appearances: p.appearances.clone(),
        draft_appearances: draft.appearances.clone(),
        looks: p.looks.clone(),
        owned_looks: data::APPEARANCES
            .iter()
            .map(|a| p.appearance_owned(a))
            .collect(),
        icons: displays.is_some(),
        resets: p.perk_resets,
        section: draft.section,
        lang: *lang_seen,
    };
    if shown.as_ref() == Some(&view) {
        return;
    }
    *shown = Some(view);
    let f = &*fonts;
    let perks = draft.perks(p);
    let spent = draft.spend(p).cost();
    let earned = p.points_earned();
    let left = earned.saturating_sub(spent);
    let dirty = draft.dirty();

    commands.entity(side).despawn_related::<Children>();
    commands.entity(side).with_children(|s| {
        s.spawn(Node {
            align_items: AlignItems::FlexEnd,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|r| {
            r.spawn(kit::text(&f.number, left.to_string(), 44.0, Color::WHITE));
            r.spawn(kit::text(&f.body, tr("points to spend"), 16.0, theme::TEXT));
        });
        s.spawn(kit::text(
            &f.body,
            trf(
                "{} earned from achievements · {} spent",
                &[&earned.to_string(), &spent.to_string()],
            ),
            13.0,
            theme::DIM,
        ));
        s.spawn(Node {
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|r| {
            for (label, action) in [
                (tr("Confirm"), RewardAction::Confirm),
                (tr("Undo"), RewardAction::Undo),
            ] {
                let b = kit::button(r, f, label, None, kit::ButtonLook::PLAIN, action);
                r.commands().entity(b).insert(kit::Disabled(!dirty));
            }
        });
        for (section, label) in SECTIONS {
            let b = kit::button(
                s,
                f,
                tr(label),
                None,
                kit::ButtonLook::PLAIN,
                RewardAction::Section(section),
            );
            let built = matches!(section, Section::Perks | Section::Races | Section::Tabards | Section::Appearances);
            s.commands().entity(b).insert((
                kit::Selected(section == draft.section),
                kit::Disabled(!built),
                Node {
                    width: Val::Percent(100.0),
                    align_items: AlignItems::FlexStart,
                    padding: UiRect::axes(Val::Px(14.0), Val::Px(8.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
            ));
            if !built {
                s.commands()
                    .entity(b)
                    .insert(Tooltip::simple(tr(label), tr("Coming soon.")));
            }
        }
        kit::heading(s, f, tr("Reset"));
        let resets_left = data::MAX_PERK_RESETS.saturating_sub(p.perk_resets);
        s.spawn(kit::text(
            &f.body,
            trf(
                "Takes back every point spent on perks, to spend again. {} of {} resets left on this save.",
                &[&resets_left.to_string(), &data::MAX_PERK_RESETS.to_string()],
            ),
            13.0,
            theme::TEXT,
        ));
        let reset = kit::button(
            s,
            f,
            tr("Reset perks"),
            None,
            kit::ButtonLook::ACTION,
            RewardAction::Reset,
        );
        s.commands()
            .entity(reset)
            .insert(kit::Disabled(resets_left == 0 || p.perks.is_empty()));
        s.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        s.spawn(kit::text(
            &f.body,
            tr("Everything here is the account's: every hero of every class has it. Achievements earn more points."),
            12.0,
            theme::DIM,
        ));
    });

    commands.entity(main).despawn_related::<Children>();
    commands.entity(main).with_children(|m| match draft.section {
        Section::Races => {
            kit::heading(m, f, tr("Races"));
            m.spawn(kit::text(
                &f.body,
                trf(
                    "Open a race to every class, {} points each. An opened race stays open through perk resets.",
                    &[&data::RACE_POINTS.to_string()],
                ),
                13.0,
                theme::DIM,
            ));
            let races = draft.races(p);
            grid(m, |grid| {
                for race in data::RACES {
                    let name = crate::char_select::race_name(race);
                    let state = if p.races.contains(name) {
                        RaceState::Open
                    } else if races.contains(name) {
                        RaceState::Drafted
                    } else {
                        RaceState::Closed
                    };
                    race_card(grid, f, race, state, left);
                }
            });
        }
        Section::Tabards => {
            kit::heading(m, f, tr("Tabards"));
            m.spawn(kit::text(
                &f.body,
                tr("Worn over the gear, in the menu and in every run; only the look. Some are bought with points, the others earned by an achievement."),
                13.0,
                theme::DIM,
            ));
            let tabards = draft.tabards(p);
            let catalog = displays.as_deref().map(|d| &d.catalog);
            grid(m, |grid| {
                for (i, t) in data::TABARDS.iter().enumerate() {
                    let state = if p.tabard == t.key && p.tabard_owned(t) {
                        LookState::Worn
                    } else if p.tabard_owned(t) {
                        LookState::Owned
                    } else if tabards.contains(t.key) {
                        LookState::Drafted
                    } else {
                        LookState::Locked
                    };
                    let icon = catalog
                        .and_then(|c| c.get(t.display)?.icon.clone())
                        .unwrap_or_else(|| TABARD_ICON.to_string());
                    look_card(
                        grid,
                        f,
                        (t.name, t.unlock),
                        state,
                        icon,
                        left,
                        RewardAction::Tabard(i),
                    );
                }
            });
        }
        Section::Appearances => {
            kit::heading(m, f, tr("Appearances"));
            m.spawn(kit::text(
                &f.body,
                tr("A look worn on the head or the back over the gear's, its numbers kept; or the slot shown bare. Some are bought with points, the others earned by an achievement."),
                13.0,
                theme::DIM,
            ));
            let bought = draft.appearances(p);
            let catalog = displays.as_deref().map(|d| &d.catalog);
            for slot in data::LookSlot::ALL {
                kit::heading(
                    m,
                    f,
                    tr(match slot {
                        data::LookSlot::Head => "Head",
                        data::LookSlot::Back => "Cloak",
                    }),
                );
                grid(m, |grid| {
                    for (i, a) in data::APPEARANCES.iter().enumerate() {
                        if a.slot != slot {
                            continue;
                        }
                        let owned = p.appearance_owned(a);
                        let worn = p.looks.get(slot.name()).map(String::as_str) == Some(a.key);
                        let state = if owned && worn {
                            LookState::Worn
                        } else if owned {
                            LookState::Owned
                        } else if bought.contains(a.key) {
                            LookState::Drafted
                        } else {
                            LookState::Locked
                        };
                        let icon = catalog
                            .and_then(|c| c.get(a.display)?.icon.clone())
                            .unwrap_or_else(|| {
                                match slot {
                                    data::LookSlot::Head => HEAD_ICON,
                                    data::LookSlot::Back => BACK_ICON,
                                }
                                .to_string()
                            });
                        look_card(
                            grid,
                            f,
                            (a.name, a.unlock),
                            state,
                            icon,
                            left,
                            RewardAction::Appearance(i),
                        );
                    }
                });
            }
        }
        _ => {
            kit::heading(m, f, tr("Perks"));
            grid(m, |grid| {
                for (i, perk) in data::PERKS.iter().enumerate() {
                    let rank = perks.get(perk.key).copied().unwrap_or(0).min(perk.max_rank);
                    perk_card(grid, f, i, perk, rank, left);
                }
            });
        }
    });
}

/// The main panel's grid of cards, two to a row.
fn grid(m: &mut ChildSpawnerCommands, fill: impl FnOnce(&mut ChildSpawnerCommands)) {
    m.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: Val::Px(10.0),
        row_gap: Val::Px(10.0),
        ..default()
    })
    .with_children(fill);
}

/// A card's frame: lit once bought.
fn card_node(lit: bool) -> impl Bundle {
    (
        Node {
            width: Val::Percent(49.0),
            min_width: Val::Px(320.0),
            flex_grow: 1.0,
            align_items: AlignItems::Center,
            column_gap: Val::Px(12.0),
            padding: UiRect::all(Val::Px(10.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            ..default()
        },
        BackgroundColor(if lit { theme::BUTTON_HOT } else { theme::INSET }),
        BorderColor::all(theme::BRASS.with_alpha(0.7)),
    )
}

/// Where a race stands in the Rewards tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RaceState {
    Closed,
    /// On the draft, kept with Confirm.
    Drafted,
    Open,
}

/// A tabard's icon while the item displays are not read yet, or for a bare slot.
const TABARD_ICON: &str = "Interface\\Icons\\INV_Shirt_GuildTabard_01";
const HEAD_ICON: &str = "Interface\\PaperDoll\\UI-PaperDoll-Slot-Head";
const BACK_ICON: &str = "Interface\\PaperDoll\\UI-PaperDoll-Slot-Chest";

/// Where a tabard or an appearance stands in the Rewards tab.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LookState {
    /// Not owned: its price, or the achievement that earns it.
    Locked,
    /// On the draft, kept with Confirm.
    Drafted,
    Owned,
    Worn,
}

/// One tabard or appearance: its icon, name, how it is earned, and Buy, Wear or Take off.
fn look_card(
    p: &mut ChildSpawnerCommands,
    f: &KitFonts,
    (name, unlock): (&str, data::Unlock),
    state: LookState,
    icon: String,
    left: u32,
    action: RewardAction,
) {
    let how = match unlock {
        data::Unlock::Points(0) => tr("Free").to_string(),
        data::Unlock::Points(n) => trf("{} points", &[&n.to_string()]),
        data::Unlock::Achievement(key) => {
            let name = data::ACHIEVEMENTS
                .iter()
                .find(|a| a.key == key)
                .map_or(key, |a| a.name.as_str());
            trf("Achievement: {}", &[tr(name)])
        }
    };
    p.spawn(card_node(matches!(
        state,
        LookState::Worn | LookState::Drafted
    )))
    .with_children(|card| {
        kit::icon(card, IconSource::Texture(icon), 44.0, None);
        card.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            row_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(kit::text(&f.body, tr(name), 18.0, Color::WHITE));
            c.spawn(kit::text(&f.body, how, 13.0, theme::TEXT));
        });
        card.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|c| {
            let (label, enabled) = match (state, unlock) {
                (LookState::Worn, _) => (tr("Take off"), true),
                (LookState::Owned, _) => (tr("Wear"), true),
                (LookState::Drafted, _) => (tr("Take back"), true),
                (LookState::Locked, data::Unlock::Points(n)) => (tr("Buy"), left >= n),
                (LookState::Locked, data::Unlock::Achievement(_)) => (tr("Locked"), false),
            };
            if state == LookState::Worn {
                c.spawn(kit::text(&f.body, tr("Worn"), 14.0, theme::GREEN));
            }
            let b = kit::button(c, f, label, None, kit::ButtonLook::PLAIN, action);
            c.commands().entity(b).insert((
                kit::Selected(state == LookState::Drafted),
                kit::Disabled(!enabled),
            ));
        });
    });
}

/// One race: its face, name, the classes it opens to, and Open.
fn race_card(p: &mut ChildSpawnerCommands, f: &KitFonts, race: u8, state: RaceState, left: u32) {
    let name = tr(crate::char_select::race_name(race));
    let opens: Vec<&str> = data::race_opens(race).map(|c| tr(c.name)).collect();
    let opens = opens.join(", ");
    p.spawn(card_node(state != RaceState::Closed))
        .with_children(|card| {
            if let Some(tc) = crate::glue::art::race_tc(race, 0) {
                kit::icon(
                    card,
                    IconSource::Atlas(
                        "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Races".into(),
                        tc,
                    ),
                    44.0,
                    None,
                );
            }
            card.spawn(Node {
                flex_direction: FlexDirection::Column,
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                row_gap: Val::Px(3.0),
                ..default()
            })
            .with_children(|c| {
                c.spawn(kit::text(&f.body, name, 18.0, Color::WHITE));
                c.spawn(kit::text(
                    &f.body,
                    trf("Opens it to: {}", &[&opens]),
                    13.0,
                    theme::TEXT,
                ));
            });
            card.spawn(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                row_gap: Val::Px(4.0),
                ..default()
            })
            .with_children(|c| match state {
                RaceState::Open => {
                    c.spawn(kit::text(&f.body, tr("Opened"), 14.0, theme::GREEN));
                }
                _ => {
                    c.spawn(kit::text(
                        &f.body,
                        trf("{} points", &[&data::RACE_POINTS.to_string()]),
                        12.0,
                        theme::DIM,
                    ));
                    let drafted = state == RaceState::Drafted;
                    let b = kit::button(
                        c,
                        f,
                        if drafted { tr("Take back") } else { tr("Open") },
                        None,
                        kit::ButtonLook::PLAIN,
                        RewardAction::Race(race),
                    );
                    c.commands().entity(b).insert((
                        kit::Selected(drafted),
                        kit::Disabled(!drafted && left < data::RACE_POINTS),
                    ));
                }
            });
        });
}

/// One perk: its icon, name and rank dots, what it does at the rank shown, its price, and − +.
fn perk_card(
    p: &mut ChildSpawnerCommands,
    f: &KitFonts,
    i: usize,
    perk: &PerkDef,
    rank: u8,
    left: u32,
) {
    let maxed = rank >= perk.max_rank;
    p.spawn(card_node(rank > 0)).with_children(|card| {
        kit::icon(
            card,
            IconSource::Spell(perk.spell),
            44.0,
            Some(Tooltip(vec![
                TipLine::new(tr(perk.name), Color::WHITE).header(),
                TipLine::new(
                    trf(
                        "Rank {}/{}",
                        &[&rank.to_string(), &perk.max_rank.to_string()],
                    ),
                    theme::DIM,
                ),
                TipLine::new(
                    trf(perk.text, &[&perk.amount_at(perk.max_rank)]),
                    theme::DIM,
                )
                .right(tr("at most"), theme::DIM),
            ])),
        );
        card.spawn(Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            row_gap: Val::Px(3.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(Node {
                column_gap: Val::Px(8.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                r.spawn(kit::text(&f.body, tr(perk.name), 18.0, Color::WHITE));
                for k in 0..perk.max_rank {
                    r.spawn((
                        Node {
                            width: Val::Px(9.0),
                            height: Val::Px(9.0),
                            border_radius: BorderRadius::MAX,
                            ..default()
                        },
                        BackgroundColor(if k < rank {
                            theme::GOLD
                        } else {
                            theme::DIM.with_alpha(0.35)
                        }),
                    ));
                }
            });
            c.spawn(kit::text(
                &f.body,
                trf(perk.text, &[&perk.amount_at(rank)]),
                13.0,
                theme::TEXT,
            ));
        });
        card.spawn(Node {
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexEnd,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|c| {
            let price = if maxed {
                tr("Max rank").to_string()
            } else {
                trf("{} points", &[&perk.cost.to_string()])
            };
            c.spawn(kit::text(&f.body, price, 12.0, theme::DIM));
            c.spawn(Node {
                column_gap: Val::Px(6.0),
                ..default()
            })
            .with_children(|r| {
                let down = kit::button(
                    r,
                    f,
                    "-",
                    None,
                    kit::ButtonLook::PLAIN,
                    RewardAction::Down(i),
                );
                r.commands().entity(down).insert(kit::Disabled(rank == 0));
                let up = kit::button(r, f, "+", None, kit::ButtonLook::PLAIN, RewardAction::Up(i));
                r.commands()
                    .entity(up)
                    .insert(kit::Disabled(maxed || left < perk.cost));
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earned(points: u32) -> Profile {
        // Enough runs and wins for the points asked, by the achievements' own rule.
        let mut p = Profile::default();
        p.totals.runs = 1000;
        p.totals.wins = 1000;
        p.totals.kills = 1_000_000;
        assert!(p.points_earned() >= points, "{}", p.points_earned());
        p
    }

    #[test]
    fn points_come_from_achievements_and_perks_spend_them() {
        let fresh = Profile::default();
        assert_eq!(fresh.points_earned(), 0);
        let mut p = earned(30);
        let mut perks = BTreeMap::new();
        perks.insert("second_thoughts".to_string(), 2);
        assert!(p.set_perks(perks.clone()));
        assert_eq!(
            p.points_spent(),
            2 * data::perk("second_thoughts").unwrap().cost
        );
        assert_eq!(p.points_left(), p.points_earned() - p.points_spent());
        // Past a perk's most, or an unknown perk: refused, nothing changed.
        perks.insert("second_thoughts".to_string(), 9);
        assert!(!p.set_perks(perks));
        let mut unknown = BTreeMap::new();
        unknown.insert("no_such_perk".to_string(), 1);
        assert!(!p.set_perks(unknown));
        assert_eq!(p.perk_rank("second_thoughts"), 2);
        // More than the points earned: refused.
        let mut fresh = Profile::default();
        let mut one = BTreeMap::new();
        one.insert("banish".to_string(), 1);
        assert!(!fresh.set_perks(one));
    }

    #[test]
    fn a_reset_takes_every_perk_point_back_twice_a_save() {
        let mut p = earned(30);
        let mut perks = BTreeMap::new();
        perks.insert("banish".to_string(), 1);
        for _ in 0..data::MAX_PERK_RESETS {
            assert!(p.set_perks(perks.clone()));
            assert!(p.reset_perks());
            assert_eq!(p.points_spent(), 0);
        }
        assert!(p.set_perks(perks));
        assert!(!p.reset_perks(), "no reset left");
        assert_eq!(p.perk_rank("banish"), 1);
    }

    #[test]
    fn perks_land_on_the_hero() {
        let mut v = Vitals::new(&data::CLASSES[0], 1);
        let start = (v.rerolls, v.banishes, v.skip_gold, v.gold);
        let all: Vec<(&'static PerkDef, u8)> =
            data::PERKS.iter().map(|p| (p, p.max_rank)).collect();
        apply_perks(&mut v, all);
        assert_eq!(v.rerolls, start.0 + 3);
        assert_eq!(v.banishes, start.1 + 3);
        assert_eq!(v.skip_gold, start.2 + 10);
        assert_eq!(v.opening_hand, 1);
        assert!(v.gold > start.3);
        // The bonus perks ride the profile's bonuses.
        let mut p = earned(200);
        let mut perks = BTreeMap::new();
        perks.insert("veterans_lessons".to_string(), 2);
        assert!(p.set_perks(perks));
        let with = p.meta_bonuses(1, &data::CLASSES[0], None);
        assert!(with.iter().any(|b| b.stat == data::Stat::Growth));
    }

    #[test]
    fn an_opened_race_costs_its_points_and_outlives_a_reset() {
        let mut p = earned(2 * data::RACE_POINTS);
        let name = |r| crate::char_select::race_name(r).to_string();
        // Orc: no paladin in vanilla, every class once opened.
        assert!(data::race_opens(2).any(|c| c.id == 2));
        assert!(!data::race_opens(1).any(|c| c.id == 1));
        let mut races = BTreeSet::new();
        races.insert(name(2));
        assert!(p.open_races(races.clone()));
        assert!(p.race_open(2) && !p.race_open(1));
        assert_eq!(p.opened_races(), [2]);
        assert_eq!(p.points_spent(), data::RACE_POINTS);
        // Closing one again, or naming no race: refused.
        assert!(!p.open_races(BTreeSet::new()));
        assert!(!p.open_races(BTreeSet::from(["Murloc".to_string(), name(2)])));
        // Perks and races share the points.
        let left = p.points_left();
        let perk = data::perk("banish").unwrap();
        let mut perks = BTreeMap::new();
        perks.insert(perk.key.to_string(), 1);
        assert_eq!(p.set_perks(perks.clone()), perk.cost <= left);
        // A reset gives the perks back and leaves the race.
        p.perks = perks;
        assert!(p.reset_perks());
        assert!(p.race_open(2));
        assert_eq!(p.points_spent(), data::RACE_POINTS);
        // Past the points earned: refused.
        let mut poor = Profile::default();
        assert!(!poor.open_races(races));
    }

    #[test]
    fn an_opened_race_is_every_class() {
        // A mage's looks without an Orc; the Orc opened adds one in each sex.
        let mage = vec![(100, 1, 0), (101, 1, 1), (102, 5, 0)];
        let looks = super::super::menu::open_looks(mage.clone(), &[2]);
        assert!(looks.contains(&(100, 2, 0)) && looks.contains(&(101, 2, 1)));
        assert_eq!(looks.len(), 5);
        // A race the class already has keeps its own looks.
        assert_eq!(super::super::menu::open_looks(mage.clone(), &[1]), mage);
        // No looks at all: nothing to borrow.
        assert!(super::super::menu::open_looks(Vec::new(), &[2]).is_empty());
    }

    #[test]
    fn tabards_are_bought_or_earned_and_one_is_worn() {
        let mut p = Profile::default();
        let bought = data::tabard("stormwind").unwrap();
        let earned_one = data::tabard("frostwolf").unwrap();
        // Nothing owned, nothing worn.
        assert!(!p.tabard_owned(bought) && !p.tabard_owned(earned_one));
        assert!(!p.wear_tabard(Some("stormwind")));
        assert!(!p.buy_tabards(BTreeSet::from(["stormwind".to_string()])));
        // Points buy the one for points; the achievement's is never sold.
        let mut p = earned(200);
        assert!(!p.buy_tabards(BTreeSet::from(["frostwolf".to_string()])));
        assert!(p.buy_tabards(BTreeSet::from(["stormwind".to_string()])));
        assert_eq!(p.points_spent(), 50);
        // The victories earned the Frostwolf tabard.
        assert!(p.tabard_owned(earned_one));
        assert!(p.wear_tabard(Some("frostwolf")));
        assert_eq!(p.worn_tabard().map(|t| t.display), Some(earned_one.display));
        assert!(p.wear_tabard(Some("stormwind")));
        assert_eq!(p.worn_tabard().map(|t| t.key), Some("stormwind"));
        assert!(p.wear_tabard(None));
        assert!(p.worn_tabard().is_none());
        // Bought stays bought: a smaller set is refused, a perk reset leaves it.
        assert!(!p.buy_tabards(BTreeSet::new()));
        p.perks.insert("banish".into(), 1);
        assert!(p.reset_perks());
        assert!(p.tabard_owned(bought));
        // A tabard the profile lost hold of is not worn.
        p.tabard = "no_such_tabard".into();
        assert!(p.worn_tabard().is_none());
    }

    #[test]
    fn appearances_are_bought_or_earned_and_one_a_slot_is_worn() {
        use data::LookSlot::{Back, Head};
        let mut p = earned(300);
        // A bare slot is free; a hat for points is not until bought; a feat's needs the feat.
        let bare = data::appearance("bare_head").unwrap();
        let hat = data::appearance("lucky_fishing_hat").unwrap();
        let wrath = data::appearance("helm_of_wrath").unwrap();
        assert!(p.appearance_owned(bare));
        assert!(!p.appearance_owned(hat));
        // Champion: 25 wins, which `earned` has.
        assert!(p.appearance_owned(wrath));
        assert!(!p.wear_appearance(Head, Some(hat.key)));
        // Bought on its own slot only, never one a feat earns.
        assert!(!p.buy_appearances(BTreeSet::from([wrath.key.to_string()])));
        assert!(p.buy_appearances(BTreeSet::from([hat.key.to_string()])));
        assert_eq!(p.points_spent(), 50);
        assert!(!p.wear_appearance(Back, Some(hat.key)), "a hat on the back");
        assert!(p.wear_appearance(Head, Some(hat.key)));
        assert!(p.wear_appearance(Back, Some("bare_back")));
        let worn: Vec<&str> = p.worn_appearances().iter().map(|a| a.key).collect();
        assert_eq!(worn, ["lucky_fishing_hat", "bare_back"]);
        // Over the gear: the hat on the head, the back bared, the tabard last.
        assert!(p.wear_tabard(Some("frostwolf")));
        use super::super::outfit::slot;
        assert_eq!(
            p.cosmetics(),
            [
                (slot::HEAD, hat.display),
                (slot::BACK, 0),
                (slot::TABARD, data::tabard("frostwolf").unwrap().display)
            ]
        );
        // Another on the same slot replaces it; None takes it off.
        assert!(p.wear_appearance(Head, Some(wrath.key)));
        assert!(p.wear_appearance(Back, None));
        assert_eq!(p.worn_appearances().len(), 1);
        // A perk reset keeps what was bought.
        p.perks.insert("banish".into(), 1);
        assert!(p.reset_perks());
        assert!(p.appearance_owned(hat));
    }

    #[test]
    fn a_guests_cosmetics_keep_one_a_slot() {
        use super::super::outfit::slot;
        let c = super::super::profile::cosmetics(
            [
                "helm_of_wrath",
                "lucky_fishing_hat",
                "no_such_look",
                "bare_back",
            ],
            Some("no_such_tabard"),
        );
        assert_eq!(c, [(slot::HEAD, 34215), (slot::BACK, 0)]);
    }

    #[test]
    fn every_appearance_is_sound() {
        let mut keys = std::collections::HashSet::new();
        for a in data::APPEARANCES {
            assert!(keys.insert(a.key), "{} twice", a.key);
            assert!(a.display < 0x1_0000, "{}", a.key);
            // Only a bare slot is free, and it has no display.
            assert_eq!(
                a.display == 0,
                a.unlock == data::Unlock::Points(0),
                "{}",
                a.key
            );
            if let data::Unlock::Achievement(x) = a.unlock {
                assert!(
                    data::ACHIEVEMENTS.iter().any(|y| y.key == x),
                    "{}: no achievement {x}",
                    a.key
                );
            }
        }
        for slot in data::LookSlot::ALL {
            assert!(data::APPEARANCES
                .iter()
                .any(|a| a.slot == slot && a.display == 0));
        }
    }

    #[test]
    fn every_tabard_is_sound() {
        let mut keys = std::collections::HashSet::new();
        for t in data::TABARDS {
            assert!(keys.insert(t.key), "{} twice", t.key);
            assert!(t.display > 0 && t.display < 0x1_0000, "{}", t.key);
            if let data::Unlock::Achievement(a) = t.unlock {
                assert!(
                    data::ACHIEVEMENTS.iter().any(|x| x.key == a),
                    "{}: no achievement {a}",
                    t.key
                );
            }
        }
    }

    #[test]
    fn a_tabard_is_worn_over_the_body() {
        // Without the catalog the hero is the trainer: nothing to wear it on.
        let class = &data::CLASSES[0];
        let body = super::super::outfit::body(
            class,
            1,
            0,
            [0; 5],
            7,
            &[],
            &[(super::super::outfit::slot::TABARD, 21338)],
            None,
            None,
        );
        assert_eq!(body, super::super::outfit::Body::Trainer { display: 7 });
    }

    #[test]
    fn every_perk_is_sound() {
        let mut keys = std::collections::HashSet::new();
        for p in data::PERKS {
            assert!(keys.insert(p.key), "{} twice", p.key);
            assert!(p.max_rank > 0 && p.cost > 0, "{}", p.key);
            assert!(p.text.contains("{}"), "{}", p.key);
        }
    }
}
