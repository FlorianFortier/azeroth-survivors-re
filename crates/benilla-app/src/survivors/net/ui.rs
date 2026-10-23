//! The multiplayer screens: the menu's way in, the lobby (create, join by code, browse, the
//! table, the host's rules, ready and start), the party frames in a run, and the scoreboard at
//! its end.
//!
//! The lobby panel is rebuilt whenever what it shows changes (a cheap signature compared each
//! frame), so every row is plain Bevy UI drawn from [`Net`] and [`LobbyUi`].

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;
use survivors_net::protocol::{normalize_code, LobbyState, LobbyView, Up, CODE_LEN};
use survivors_net::settings::{
    clean_name, Difficulty, Settings, BOSS_EVERY_MAX, BOSS_EVERY_MIN, DURATION_MAX, DURATION_MIN,
    MAX_LOBBY_NAME, MAX_PLAYERS, MAX_PLAYER_NAME, PERCENT_MAX, PERCENT_MIN,
};

use super::{loadout, Net, Party, Status};
use crate::survivors::hero::REVIVE_SECS;
use crate::survivors::hud::HudFont;
use crate::survivors::{data, Phase, Run, Selection, Session};

const GOLD: Color = Color::srgb(1.0, 0.82, 0.0);
const EDGE: Color = Color::srgb(0.55, 0.45, 0.25);
const DIM: Color = Color::srgb(0.6, 0.6, 0.6);
const RED: Color = Color::srgb(1.0, 0.35, 0.3);
const GREEN: Color = Color::srgb(0.35, 1.0, 0.35);
const BUTTON: Color = Color::srgba(0.18, 0.06, 0.04, 0.95);
const BUTTON_HOT: Color = Color::srgba(0.36, 0.12, 0.06, 0.98);
const BUTTON_OFF: Color = Color::srgba(0.12, 0.1, 0.09, 0.85);
const FIELD: Color = Color::srgba(0.02, 0.02, 0.02, 0.9);
const FIELD_FOCUS: Color = Color::srgba(0.08, 0.06, 0.02, 0.95);

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<LobbyUi>()
        .add_systems(Startup, setup.after(crate::survivors::hud::setup_hud))
        .add_systems(
            Update,
            (
                type_into_fields,
                clicks,
                rebuild_lobby,
                show_roots,
                refresh_party,
                refresh_down,
                refresh_scores,
            )
                .chain()
                .after(crate::survivors::hud::show_panels),
        );
}

/// A text field on the lobby screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Field {
    Name,
    Server,
    Code,
    LobbyName,
}

/// A host's change to one rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rule {
    Zone(i8),
    Seats(i8),
    Preset(i8),
    Health(i8),
    Damage(i8),
    Density(i8),
    Scale,
    Duration(i8),
    BossEvery(i8),
    SharedXp,
    Revive,
    Listed,
}

#[derive(Component, Clone, Debug, PartialEq)]
enum Action {
    /// The menu's way in, or back to the lobby the player sits in.
    Open,
    Back,
    Create,
    Join,
    JoinCode(String),
    Browse,
    Focus(Field),
    Leave,
    Ready,
    Start,
    Kick(u8),
    ChangeHero,
    Set(Rule),
    CopyCode,
}

/// The colour a button rests at.
#[derive(Component)]
struct Rest(Color);

/// The lobby screen's own state: the field being typed in and the drafts.
#[derive(Resource)]
pub(super) struct LobbyUi {
    focus: Option<Field>,
    name: String,
    server: String,
    code: String,
    lobby_name: String,
    browsed: bool,
    signature: String,
}

impl Default for LobbyUi {
    fn default() -> Self {
        Self {
            focus: None,
            name: String::new(),
            server: String::new(),
            code: String::new(),
            lobby_name: String::new(),
            browsed: false,
            signature: String::from("-"),
        }
    }
}

#[derive(Component)]
struct LobbyRoot;
#[derive(Component)]
struct LobbyPanel;
#[derive(Component)]
struct OpenButton;
#[derive(Component)]
struct OpenLabel;
#[derive(Component)]
struct PartyRoot;
#[derive(Component)]
struct DownLine;
#[derive(Component)]
struct ScoreRoot;

fn text(f: &Handle<Font>, s: impl Into<String>, size: f32, color: Color) -> impl Bundle {
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
}

fn button(p: &mut ChildSpawnerCommands, f: &Handle<Font>, label: &str, action: Action, on: bool) {
    let base = if on { BUTTON } else { BUTTON_OFF };
    let mut b = p.spawn((
        Node {
            padding: UiRect::axes(Val::Px(14.0), Val::Px(7.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(4.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(base),
        BorderColor::all(if on { EDGE } else { DIM.with_alpha(0.4) }),
    ));
    if on {
        b.insert((Button, action, Rest(base)));
    }
    b.with_children(|b| {
        b.spawn(text(f, label, 15.0, if on { GOLD } else { DIM }));
    });
}

fn small(p: &mut ChildSpawnerCommands, f: &Handle<Font>, label: &str, action: Action) {
    p.spawn((
        Button,
        action,
        Rest(BUTTON),
        Node {
            width: Val::Px(26.0),
            height: Val::Px(24.0),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(BUTTON),
        BorderColor::all(EDGE),
    ))
    .with_children(|b| {
        b.spawn(text(f, label, 14.0, GOLD));
    });
}

fn field(
    p: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    value: &str,
    which: Field,
    ui: &LobbyUi,
    width: f32,
) {
    let focused = ui.focus == Some(which);
    let shown = if focused {
        format!("{value}|")
    } else if value.is_empty() {
        String::from(" ")
    } else {
        value.to_string()
    };
    p.spawn((
        Button,
        Action::Focus(which),
        Rest(if focused { FIELD_FOCUS } else { FIELD }),
        Node {
            width: Val::Px(width),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(5.0)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::all(Val::Px(3.0)),
            ..default()
        },
        BackgroundColor(if focused { FIELD_FOCUS } else { FIELD }),
        BorderColor::all(if focused { GOLD } else { EDGE }),
    ))
    .with_children(|b| {
        b.spawn(text(f, shown, 15.0, Color::WHITE));
    });
}

fn row(p: &mut ChildSpawnerCommands, build: impl FnOnce(&mut ChildSpawnerCommands)) {
    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        align_items: AlignItems::Center,
        column_gap: Val::Px(8.0),
        ..default()
    })
    .with_children(build);
}

fn setup(mut commands: Commands, font: Res<HudFont>) {
    let f = &font.0;
    // The menu's way in, bottom left.
    commands
        .spawn((
            OpenButton,
            Button,
            Action::Open,
            Rest(BUTTON),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(24.0),
                bottom: Val::Px(24.0),
                padding: UiRect::axes(Val::Px(18.0), Val::Px(9.0)),
                border: UiRect::all(Val::Px(2.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(BUTTON),
            BorderColor::all(GOLD),
            GlobalZIndex(460),
            Visibility::Hidden,
        ))
        .with_children(|b| {
            b.spawn((OpenLabel, text(f, "Multiplayer", 18.0, GOLD)));
        });
    // The lobby screen.
    commands
        .spawn((
            LobbyRoot,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
            GlobalZIndex(450),
            Visibility::Hidden,
        ))
        .with_children(|root| {
            root.spawn((
                LobbyPanel,
                Node {
                    width: Val::Px(860.0),
                    max_width: Val::Percent(96.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::all(Val::Px(22.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    border_radius: BorderRadius::all(Val::Px(8.0)),
                    ..default()
                },
                BackgroundGradient::from(LinearGradient::to_bottom(vec![
                    ColorStop::percent(Color::srgba(0.16, 0.11, 0.05, 0.96), 0.0),
                    ColorStop::percent(Color::srgba(0.04, 0.03, 0.02, 0.97), 100.0),
                ])),
                BorderColor::all(EDGE),
            ));
        });
    // The party frames, left middle.
    commands.spawn((
        PartyRoot,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(16.0),
            top: Val::Percent(30.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(420),
        Visibility::Hidden,
    ));
    // A fallen hero's line, and the wait for the others to load.
    commands.spawn((
        DownLine,
        text(f, "", 22.0, GOLD),
        TextLayout::new_with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(22.0),
            width: Val::Percent(100.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(430),
        Visibility::Hidden,
    ));
    // The table's scores, under the results.
    commands.spawn((
        ScoreRoot,
        Node {
            position_type: PositionType::Absolute,
            bottom: Val::Px(28.0),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(4.0),
            display: Display::None,
            ..default()
        },
        GlobalZIndex(445),
        Visibility::Hidden,
    ));
}

fn show(v: &mut Visibility, node: &mut Node, on: bool) {
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
}

#[allow(clippy::type_complexity)]
fn show_roots(
    phase: Res<State<Phase>>,
    session: Res<Session>,
    mut roots: ParamSet<(
        Query<(&mut Visibility, &mut Node), With<OpenButton>>,
        Query<(&mut Visibility, &mut Node), With<LobbyRoot>>,
        Query<(&mut Visibility, &mut Node), With<PartyRoot>>,
        Query<(&mut Visibility, &mut Node), With<ScoreRoot>>,
    )>,
    mut label: Query<&mut Text, With<OpenLabel>>,
) {
    let p = *phase.get();
    for (mut v, mut n) in &mut roots.p0() {
        show(&mut v, &mut n, p == Phase::Menu);
    }
    for (mut v, mut n) in &mut roots.p1() {
        show(&mut v, &mut n, p == Phase::Lobby);
    }
    let in_run = matches!(p, Phase::Playing | Phase::LevelUp | Phase::Paused);
    for (mut v, mut n) in &mut roots.p2() {
        show(&mut v, &mut n, in_run && session.multiplayer());
    }
    for (mut v, mut n) in &mut roots.p3() {
        show(
            &mut v,
            &mut n,
            p == Phase::GameOver && session.multiplayer(),
        );
    }
    let want = if session.in_lobby {
        "Back to lobby"
    } else {
        "Multiplayer"
    };
    for mut t in &mut label {
        if t.0 != want {
            t.0 = want.to_string();
        }
    }
}

/// Typing into the focused field; Enter submits it, Tab moves on, Escape lets go.
#[allow(clippy::too_many_arguments)]
fn type_into_fields(
    phase: Res<State<Phase>>,
    mut keys: MessageReader<KeyboardInput>,
    mut ui: ResMut<LobbyUi>,
    mut net: ResMut<Net>,
    selection: Res<Selection>,
    mut next: ResMut<NextState<Phase>>,
) {
    if *phase.get() != Phase::Lobby {
        keys.clear();
        return;
    }
    if ui.name.is_empty() && ui.server.is_empty() {
        ui.name.clone_from(&net.name);
        ui.server.clone_from(&net.server);
    }
    for ev in keys.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        let Some(focus) = ui.focus else {
            if ev.logical_key == Key::Escape {
                next.set(Phase::Menu);
            }
            continue;
        };
        match &ev.logical_key {
            Key::Escape => {
                commit(focus, &mut ui, &mut net);
                ui.focus = None;
            }
            Key::Enter => {
                commit(focus, &mut ui, &mut net);
                ui.focus = None;
                if focus == Field::Code {
                    join(&ui.code.clone(), &mut net, &selection);
                }
            }
            Key::Tab => {
                commit(focus, &mut ui, &mut net);
                ui.focus = Some(match focus {
                    Field::Name => Field::Server,
                    Field::Server => Field::Code,
                    Field::Code => Field::Name,
                    Field::LobbyName => Field::LobbyName,
                });
            }
            Key::Backspace => {
                buffer(focus, &mut ui).pop();
            }
            _ => {
                if let Some(t) = &ev.text {
                    let limit = match focus {
                        Field::Name => MAX_PLAYER_NAME,
                        Field::Server => 200,
                        Field::Code => CODE_LEN + 2,
                        Field::LobbyName => MAX_LOBBY_NAME,
                    };
                    let buf = buffer(focus, &mut ui);
                    for c in t.chars().filter(|c| !c.is_control()) {
                        if buf.chars().count() < limit {
                            buf.push(if focus == Field::Code {
                                c.to_ascii_uppercase()
                            } else {
                                c
                            });
                        }
                    }
                }
            }
        }
    }
}

fn buffer(f: Field, ui: &mut LobbyUi) -> &mut String {
    match f {
        Field::Name => &mut ui.name,
        Field::Server => &mut ui.server,
        Field::Code => &mut ui.code,
        Field::LobbyName => &mut ui.lobby_name,
    }
}

/// A field's text takes effect: a new name or server reconnects; a lobby name is the host's rule.
fn commit(f: Field, ui: &mut LobbyUi, net: &mut Net) {
    match f {
        Field::Name => {
            let name = clean_name(&ui.name, MAX_PLAYER_NAME);
            if !name.is_empty() && name != net.name {
                net.name = name.clone();
                // The server knows a player by the name they said hello with.
                if net.lobby.is_none() {
                    net.disconnect();
                }
            }
            ui.name = net.name.clone();
        }
        Field::Server => {
            let server = ui.server.trim().to_string();
            if !server.is_empty() && server != net.server {
                if let Some(_lobby) = &net.lobby {
                    net.send(Up::Leave);
                }
                net.disconnect();
                net.server = server;
            }
            ui.server = net.server.clone();
        }
        Field::Code => {}
        Field::LobbyName => {
            if let Some(view) = &net.lobby {
                if view.is_host() {
                    let mut s = view.settings.clone();
                    s.name = clean_name(&ui.lobby_name, MAX_LOBBY_NAME);
                    if !s.name.is_empty() && s.name != view.settings.name {
                        net.send(Up::UpdateSettings(s));
                    }
                }
            }
        }
    }
}

fn join(code: &str, net: &mut Net, selection: &Selection) {
    let code = normalize_code(code);
    if code.len() == CODE_LEN {
        net.send(Up::Join {
            code,
            loadout: loadout(selection),
        });
    } else {
        net.notice = Some(format!("A lobby code has {CODE_LEN} characters."));
    }
}

fn step_pct(v: u16, d: i8) -> u16 {
    (i32::from(v) + i32::from(d) * 10).clamp(i32::from(PERCENT_MIN), i32::from(PERCENT_MAX)) as u16
}

fn apply_rule(s: &Settings, rule: Rule) -> Settings {
    let mut s = s.clone();
    let presets = [
        Difficulty::EASY,
        Difficulty::NORMAL,
        Difficulty::HARD,
        Difficulty::NIGHTMARE,
    ];
    match rule {
        Rule::Zone(d) => {
            let n = data::MAPS.len() as i32;
            s.zone = (i32::from(s.zone) + i32::from(d)).rem_euclid(n) as u8;
        }
        Rule::Seats(d) => {
            s.max_players =
                (i32::from(s.max_players) + i32::from(d)).clamp(1, i32::from(MAX_PLAYERS)) as u8;
        }
        Rule::Preset(d) => {
            let at = presets
                .iter()
                .position(|p| {
                    p.enemy_health == s.difficulty.enemy_health
                        && p.enemy_damage == s.difficulty.enemy_damage
                        && p.density == s.difficulty.density
                })
                .map_or(1, |i| i as i32);
            let i = (at + i32::from(d)).rem_euclid(presets.len() as i32) as usize;
            let scale = s.difficulty.scale_with_players;
            s.difficulty = presets[i];
            s.difficulty.scale_with_players = scale;
        }
        Rule::Health(d) => s.difficulty.enemy_health = step_pct(s.difficulty.enemy_health, d),
        Rule::Damage(d) => s.difficulty.enemy_damage = step_pct(s.difficulty.enemy_damage, d),
        Rule::Density(d) => s.difficulty.density = step_pct(s.difficulty.density, d),
        Rule::Scale => s.difficulty.scale_with_players = !s.difficulty.scale_with_players,
        Rule::Duration(d) => {
            s.duration_secs = (i32::from(s.duration_secs) + i32::from(d) * 60)
                .clamp(i32::from(DURATION_MIN), i32::from(DURATION_MAX))
                as u16;
        }
        Rule::BossEvery(d) => {
            s.boss_every_secs = (i32::from(s.boss_every_secs) + i32::from(d) * 15)
                .clamp(i32::from(BOSS_EVERY_MIN), i32::from(BOSS_EVERY_MAX))
                as u16;
        }
        Rule::SharedXp => s.shared_xp = !s.shared_xp,
        Rule::Revive => s.revive = !s.revive,
        Rule::Listed => s.listed = !s.listed,
    }
    s
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn clicks(
    mut buttons: Query<(&Interaction, &Action, &Rest, &mut BackgroundColor), Changed<Interaction>>,
    mut ui: ResMut<LobbyUi>,
    mut net: ResMut<Net>,
    selection: Res<Selection>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
) {
    for (interaction, action, rest, mut bg) in &mut buttons {
        match interaction {
            Interaction::Hovered => bg.0 = BUTTON_HOT.mix(&rest.0, 0.3),
            Interaction::None => bg.0 = rest.0,
            Interaction::Pressed => {
                bg.0 = BUTTON_HOT;
                if let Some(f) = ui.focus {
                    if !matches!(action, Action::Focus(g) if *g == f) {
                        commit(f, &mut ui, &mut net);
                        ui.focus = None;
                    }
                }
                match action {
                    Action::Open => {
                        if *phase.get() == Phase::Menu {
                            next.set(Phase::Lobby);
                        }
                    }
                    Action::Back | Action::ChangeHero => next.set(Phase::Menu),
                    Action::Create => {
                        let settings = Settings {
                            name: format!("{}'s run", net.name),
                            ..Settings::default()
                        };
                        net.send(Up::Create {
                            settings,
                            loadout: loadout(&selection),
                        });
                    }
                    Action::Join => join(&ui.code.clone(), &mut net, &selection),
                    Action::JoinCode(code) => join(code, &mut net, &selection),
                    Action::Browse => {
                        ui.browsed = true;
                        net.send(Up::List);
                    }
                    Action::Focus(f) => {
                        if *f == Field::LobbyName {
                            if let Some(v) = &net.lobby {
                                ui.lobby_name = v.settings.name.clone();
                            }
                        }
                        ui.focus = Some(*f);
                    }
                    Action::Leave => {
                        net.send(Up::Leave);
                        net.lobby = None;
                    }
                    Action::Ready => {
                        let ready = net
                            .lobby
                            .as_ref()
                            .and_then(LobbyView::me)
                            .is_some_and(|m| m.ready);
                        net.send(Up::SetReady(!ready));
                    }
                    Action::Start => net.send(Up::Start),
                    Action::Kick(slot) => net.send(Up::Kick(*slot)),
                    Action::Set(rule) => {
                        if let Some(view) = &net.lobby {
                            if view.is_host() && view.state == LobbyState::Open {
                                let s = apply_rule(&view.settings, *rule);
                                net.send(Up::UpdateSettings(s));
                            }
                        }
                    }
                    Action::CopyCode =>
                    {
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some(view) = &net.lobby {
                            match arboard::Clipboard::new()
                                .and_then(|mut c| c.set_text(view.code.clone()))
                            {
                                Ok(()) => net.notice = Some("Code copied.".into()),
                                Err(_) => net.notice = Some("Could not copy the code.".into()),
                            }
                        }
                    }
                }
            }
        }
    }
}

fn mmss(secs: u16) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn on_off(b: bool) -> &'static str {
    if b {
        "On"
    } else {
        "Off"
    }
}

fn class_name(i: u8) -> &'static str {
    data::CLASSES.get(usize::from(i)).map_or("?", |c| c.name)
}

fn class_color(i: u8) -> Color {
    data::CLASSES
        .get(usize::from(i))
        .map_or(Color::WHITE, |c| crate::survivors::hud::rgb(c.color))
}

/// What the panel shows, as text: when it changes, the panel is rebuilt.
fn signature(ui: &LobbyUi, net: &Net) -> String {
    format!(
        "{:?}|{}|{}|{}|{}|{}|{:?}|{:?}|{:?}|{:?}|{}",
        ui.focus,
        ui.name,
        ui.server,
        ui.code,
        ui.lobby_name,
        ui.browsed,
        net.status,
        net.lobby,
        net.lobbies,
        net.notice,
        net.name,
    )
}

fn rebuild_lobby(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    font: Res<HudFont>,
    mut ui: ResMut<LobbyUi>,
    net: Res<Net>,
    panel: Query<Entity, With<LobbyPanel>>,
) {
    if *phase.get() != Phase::Lobby {
        ui.signature.clear();
        return;
    }
    let sig = signature(&ui, &net);
    if sig == ui.signature {
        return;
    }
    ui.signature = sig;
    let Ok(panel) = panel.single() else { return };
    let f = &font.0;
    commands.entity(panel).despawn_related::<Children>();
    commands.entity(panel).with_children(|p| match &net.lobby {
        None => build_home(p, f, &ui, &net),
        Some(view) => build_table(p, f, &ui, &net, view),
    });
}

fn build_home(p: &mut ChildSpawnerCommands, f: &Handle<Font>, ui: &LobbyUi, net: &Net) {
    p.spawn(text(f, "Multiplayer", 30.0, GOLD));
    p.spawn(text(
        f,
        "Play a run together: one player hosts a lobby, the others join it with its code.",
        15.0,
        DIM,
    ));
    row(p, |r| {
        r.spawn((
            text(f, "Your name", 15.0, Color::WHITE),
            Node {
                width: Val::Px(110.0),
                ..default()
            },
        ));
        field(r, f, &ui.name, Field::Name, ui, 260.0);
    });
    row(p, |r| {
        r.spawn((
            text(f, "Server", 15.0, Color::WHITE),
            Node {
                width: Val::Px(110.0),
                ..default()
            },
        ));
        field(r, f, &ui.server, Field::Server, ui, 420.0);
        let (s, c) = match net.status {
            Status::Offline => ("offline", DIM),
            Status::Connecting => ("connecting…", GOLD),
            Status::Online => ("connected", GREEN),
        };
        r.spawn(text(f, s, 14.0, c));
    });
    row(p, |r| {
        button(r, f, "Create a lobby", Action::Create, true);
        r.spawn(Node {
            width: Val::Px(24.0),
            ..default()
        });
        r.spawn(text(f, "or join with a code", 15.0, DIM));
        field(r, f, &ui.code, Field::Code, ui, 110.0);
        button(r, f, "Join", Action::Join, !ui.code.is_empty());
    });
    row(p, |r| {
        button(r, f, "Browse public lobbies", Action::Browse, true);
    });
    if ui.browsed {
        if net.lobbies.is_empty() {
            p.spawn(text(f, "No public lobby is open right now.", 14.0, DIM));
        }
        for l in &net.lobbies {
            let zone = data::MAPS.get(usize::from(l.zone)).map_or("?", |m| m.name);
            row(p, |r| {
                r.spawn((
                    text(
                        f,
                        format!(
                            "{}  —  {}  ·  {}  ·  {}/{}  ·  {}",
                            l.name, l.host, zone, l.players, l.max_players, l.difficulty
                        ),
                        15.0,
                        Color::WHITE,
                    ),
                    Node {
                        width: Val::Px(620.0),
                        ..default()
                    },
                ));
                button(r, f, "Join", Action::JoinCode(l.code.clone()), true);
            });
        }
    }
    if let Some(n) = &net.notice {
        p.spawn(text(f, n.clone(), 15.0, RED));
    }
    row(p, |r| {
        button(r, f, "Back", Action::Back, true);
    });
}

fn build_table(
    p: &mut ChildSpawnerCommands,
    f: &Handle<Font>,
    ui: &LobbyUi,
    net: &Net,
    view: &LobbyView,
) {
    let host = view.is_host();
    let open = view.state == LobbyState::Open;
    let s = &view.settings;
    row(p, |r| {
        if host && open {
            field(
                r,
                f,
                if ui.focus == Some(Field::LobbyName) {
                    &ui.lobby_name
                } else {
                    &s.name
                },
                Field::LobbyName,
                ui,
                360.0,
            );
        } else {
            r.spawn(text(f, s.name.clone(), 26.0, GOLD));
        }
        r.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        r.spawn(text(f, "Code", 15.0, DIM));
        r.spawn(text(f, view.code.clone(), 32.0, Color::WHITE));
        #[cfg(not(target_arch = "wasm32"))]
        button(r, f, "Copy", Action::CopyCode, true);
    });
    p.spawn(text(
        f,
        if host {
            "Share the code with your friends. Start the run once everyone is ready."
        } else {
            "The host sets the rules and starts the run."
        },
        14.0,
        DIM,
    ));

    p.spawn(Node {
        flex_direction: FlexDirection::Row,
        column_gap: Val::Px(24.0),
        ..default()
    })
    .with_children(|cols| {
        // The table.
        cols.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(6.0),
            width: Val::Px(380.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(text(
                f,
                format!("Players {}/{}", view.members.len(), s.max_players),
                18.0,
                GOLD,
            ));
            for slot in 0..s.max_players {
                let member = view.members.iter().find(|m| m.slot == slot);
                row(c, |r| match member {
                    Some(m) => {
                        let tag = if m.slot == 0 { "★ " } else { "" };
                        let you = if m.slot == view.you { " (you)" } else { "" };
                        r.spawn((
                            text(f, format!("{tag}{}{you}", m.name), 16.0, Color::WHITE),
                            Node {
                                width: Val::Px(170.0),
                                ..default()
                            },
                        ));
                        r.spawn((
                            text(
                                f,
                                class_name(m.loadout.class),
                                15.0,
                                class_color(m.loadout.class),
                            ),
                            Node {
                                width: Val::Px(80.0),
                                ..default()
                            },
                        ));
                        let (st, col) = if m.slot == 0 {
                            ("host", GOLD)
                        } else if m.ready {
                            ("ready", GREEN)
                        } else {
                            ("not ready", DIM)
                        };
                        r.spawn(text(f, st, 14.0, col));
                        if host && m.slot != 0 && open {
                            small(r, f, "×", Action::Kick(m.slot));
                        }
                    }
                    None => {
                        r.spawn(text(f, "— open seat —", 15.0, DIM));
                    }
                });
            }
        });
        // The rules.
        cols.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|c| {
            c.spawn(text(f, "Rules", 18.0, GOLD));
            let zone = data::MAPS.get(usize::from(s.zone)).map_or("?", |m| m.name);
            let edit = host && open;
            let mut line = |label: &str, value: String, minus: Option<Rule>, plus: Option<Rule>| {
                row(c, |r| {
                    r.spawn((
                        text(f, label, 14.0, DIM),
                        Node {
                            width: Val::Px(150.0),
                            ..default()
                        },
                    ));
                    if edit {
                        if let Some(m) = minus {
                            small(r, f, "<", Action::Set(m));
                        }
                    }
                    r.spawn((
                        text(f, value, 15.0, Color::WHITE),
                        Node {
                            min_width: Val::Px(120.0),
                            ..default()
                        },
                    ));
                    if edit {
                        if let Some(pl) = plus {
                            small(
                                r,
                                f,
                                if minus.is_some() { ">" } else { "⇄" },
                                Action::Set(pl),
                            );
                        }
                    }
                });
            };
            line(
                "Zone",
                zone.to_string(),
                Some(Rule::Zone(-1)),
                Some(Rule::Zone(1)),
            );
            line(
                "Seats",
                s.max_players.to_string(),
                Some(Rule::Seats(-1)),
                Some(Rule::Seats(1)),
            );
            line(
                "Difficulty",
                s.difficulty.preset_name().to_string(),
                Some(Rule::Preset(-1)),
                Some(Rule::Preset(1)),
            );
            line(
                "Enemy health",
                format!("{}%", s.difficulty.enemy_health),
                Some(Rule::Health(-1)),
                Some(Rule::Health(1)),
            );
            line(
                "Enemy damage",
                format!("{}%", s.difficulty.enemy_damage),
                Some(Rule::Damage(-1)),
                Some(Rule::Damage(1)),
            );
            line(
                "Crowd",
                format!("{}%", s.difficulty.density),
                Some(Rule::Density(-1)),
                Some(Rule::Density(1)),
            );
            line(
                "Grows with players",
                on_off(s.difficulty.scale_with_players).into(),
                None,
                Some(Rule::Scale),
            );
            line(
                "Survive for",
                mmss(s.duration_secs),
                Some(Rule::Duration(-1)),
                Some(Rule::Duration(1)),
            );
            line(
                "A boss every",
                mmss(s.boss_every_secs),
                Some(Rule::BossEvery(-1)),
                Some(Rule::BossEvery(1)),
            );
            line(
                "Shared experience",
                on_off(s.shared_xp).into(),
                None,
                Some(Rule::SharedXp),
            );
            line(
                "Revive allies",
                on_off(s.revive).into(),
                None,
                Some(Rule::Revive),
            );
            line(
                "Public",
                (if s.listed { "listed" } else { "code only" }).into(),
                None,
                Some(Rule::Listed),
            );
        });
    });

    if !open {
        p.spawn(text(
            f,
            "The run is in progress; the lobby opens again when the host comes back.",
            15.0,
            GOLD,
        ));
    }
    if let Some(n) = &net.notice {
        p.spawn(text(f, n.clone(), 15.0, RED));
    }
    row(p, |r| {
        button(r, f, "Change hero", Action::ChangeHero, open);
        if host {
            let can = open && view.all_ready();
            button(
                r,
                f,
                if can || view.members.len() == 1 {
                    "Start the run"
                } else {
                    "Waiting for players…"
                },
                Action::Start,
                can,
            );
        } else {
            let ready = view.me().is_some_and(|m| m.ready);
            button(
                r,
                f,
                if ready { "Not ready" } else { "Ready" },
                Action::Ready,
                open,
            );
        }
        r.spawn(Node {
            flex_grow: 1.0,
            ..default()
        });
        button(r, f, "Leave lobby", Action::Leave, true);
    });
}

#[derive(Component)]
struct PartyRow;

/// The other heroes, at the left edge: name, level, health.
fn refresh_party(
    mut commands: Commands,
    session: Res<Session>,
    party: Res<Party>,
    font: Res<HudFont>,
    root: Query<Entity, With<PartyRoot>>,
    mut shown: Local<String>,
) {
    if !session.multiplayer() {
        return;
    }
    let others: Vec<_> = party
        .members
        .iter()
        .filter(|m| m.slot != session.slot)
        .collect();
    let sig = others
        .iter()
        .map(|m| {
            format!(
                "{}:{}:{}:{}:{}",
                m.slot,
                m.level,
                m.hp.round(),
                m.max_hp.round(),
                m.dead
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    if *shown == sig {
        return;
    }
    shown.clone_from(&sig);
    let Ok(root) = root.single() else { return };
    let f = &font.0;
    commands.entity(root).despawn_related::<Children>();
    commands.entity(root).with_children(|p| {
        for m in others {
            p.spawn((
                PartyRow,
                Node {
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(6.0)),
                    row_gap: Val::Px(3.0),
                    width: Val::Px(180.0),
                    border: UiRect::all(Val::Px(1.0)),
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                BorderColor::all(EDGE),
            ))
            .with_children(|c| {
                let col = data::CLASSES
                    .get(m.class)
                    .map_or(Color::WHITE, |c| crate::survivors::hud::rgb(c.color));
                c.spawn(text(f, format!("{}  ·  {}", m.name, m.level), 14.0, col));
                let frac = if m.max_hp > 0.0 {
                    (m.hp / m.max_hp).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                c.spawn((
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Px(8.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.2, 0.0, 0.0, 0.9)),
                ))
                .with_children(|bar| {
                    bar.spawn((
                        Node {
                            width: Val::Percent(frac * 100.0),
                            height: Val::Percent(100.0),
                            ..default()
                        },
                        BackgroundColor(if m.dead { DIM } else { GREEN }),
                    ));
                });
                if m.dead {
                    c.spawn(text(f, "Fallen", 12.0, RED));
                }
            });
        }
    });
}

/// The fallen hero's line, and the wait for the others to load.
fn refresh_down(
    phase: Res<State<Phase>>,
    session: Res<Session>,
    run: Res<Run>,
    mut line: Query<(&mut Text, &mut Visibility, &mut Node), With<DownLine>>,
) {
    let Ok((mut t, mut v, mut n)) = line.single_mut() else {
        return;
    };
    let in_run = matches!(phase.get(), Phase::Playing | Phase::LevelUp | Phase::Paused);
    let want = if !in_run || !session.multiplayer() || !run.begun {
        None
    } else if run.waiting {
        Some("Waiting for every player to arrive…".to_string())
    } else if run.dead_t.is_some() {
        Some(if session.rules.revive {
            let pct = (run.revive_t / REVIVE_SECS * 100.0).clamp(0.0, 100.0);
            if pct > 0.0 {
                format!("You have fallen — an ally is reviving you… {pct:.0}%")
            } else {
                "You have fallen — an ally can revive you by standing over you.".to_string()
            }
        } else {
            "You have fallen. Your party fights on.".to_string()
        })
    } else {
        None
    };
    show(&mut v, &mut n, want.is_some());
    if let Some(s) = want {
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// The table's numbers under the results.
fn refresh_scores(
    mut commands: Commands,
    phase: Res<State<Phase>>,
    session: Res<Session>,
    party: Res<Party>,
    run: Res<Run>,
    font: Res<HudFont>,
    root: Query<Entity, With<ScoreRoot>>,
    mut shown: Local<String>,
) {
    if *phase.get() != Phase::GameOver || !session.multiplayer() {
        shown.clear();
        return;
    }
    // The local hero's own line from the view, the others from the party.
    let mut rows: Vec<(String, u32, u32, f64)> = party
        .members
        .iter()
        .map(|m| {
            if m.slot == session.slot {
                (m.name.clone(), run.level, run.kills, run.damage_done)
            } else {
                (m.name.clone(), m.level, m.kills, m.damage_done)
            }
        })
        .collect();
    rows.sort_by(|a, b| b.3.total_cmp(&a.3));
    let sig = format!("{rows:?}");
    if *shown == sig {
        return;
    }
    shown.clone_from(&sig);
    let Ok(root) = root.single() else { return };
    let f = &font.0;
    commands.entity(root).despawn_related::<Children>();
    commands.entity(root).with_children(|p| {
        p.spawn(text(f, "The party", 18.0, GOLD));
        for (name, level, kills, dmg) in rows {
            p.spawn(text(
                f,
                format!(
                    "{name}   level {level}   ·   {kills} kills   ·   {:.0} damage",
                    dmg
                ),
                15.0,
                Color::WHITE,
            ));
        }
        p.spawn(text(
            f,
            "Press Enter or Play again to go back to the lobby.",
            13.0,
            DIM,
        ));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_step_within_their_bounds() {
        let s = Settings::default();
        let mut t = s.clone();
        for _ in 0..100 {
            t = apply_rule(&t, Rule::Duration(1));
        }
        assert_eq!(t.duration_secs, DURATION_MAX);
        for _ in 0..100 {
            t = apply_rule(&t, Rule::Health(-1));
        }
        assert_eq!(t.difficulty.enemy_health, PERCENT_MIN);
        // Zones wrap both ways.
        let z = apply_rule(&s, Rule::Zone(-1));
        assert_eq!(usize::from(z.zone), data::MAPS.len() - 1);
        assert_eq!(apply_rule(&z, Rule::Zone(1)).zone, 0);
        // Presets cycle and keep the scaling switch.
        let mut p = s.clone();
        p.difficulty.scale_with_players = false;
        let hard = apply_rule(&p, Rule::Preset(1));
        assert_eq!(hard.difficulty.preset_name(), "Hard");
        assert!(!hard.difficulty.scale_with_players);
        assert_eq!(apply_rule(&s, Rule::Seats(5)).max_players, MAX_PLAYERS);
        assert!(apply_rule(&s, Rule::Listed).listed);
    }

    #[test]
    fn clocks_read_as_minutes() {
        assert_eq!(mmss(135), "2:15");
        assert_eq!(mmss(900), "15:00");
    }
}
