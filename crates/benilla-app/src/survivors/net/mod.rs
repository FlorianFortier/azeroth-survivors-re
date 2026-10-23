//! Multiplayer: the lobby connection, the lobby screen, and the run streamed between its host and
//! guests.
//!
//! - [`socket`]: the WebSocket, native or browser.
//! - [`Net`]: the connection's state, the lobby as the server last described it, and the game
//!   messages relayed to this machine.
//! - [`host`]: the host plays the run and streams it ([`survivors_net::game`]); guests' heroes
//!   move as their players send.
//! - [`client`]: a guest builds the host's units as it would a server's, and steers its own hero.
//! - [`ui`]: the lobby screen and the party frames.
//!
//! The lobby server is `survivors-server` (also mounted by `wenilla-host` at `/lobby`).

mod client;
mod host;
pub(crate) mod socket;
mod ui;

use bevy::prelude::*;
use survivors_net::game::{decode_batch, encode_batch, Game};
use survivors_net::protocol::{
    Down, LeftReason, Loadout, LobbySummary, LobbyView, Target, Up, LOBBY_PATH, PROTOCOL_VERSION,
};
use survivors_net::settings::{clean_name, Settings, MAX_PLAYER_NAME};
use survivors_net::Wire;

use self::socket::{Socket, SocketEvent};
use super::data::{self, Rules};
use super::{Mode, Phase, Rng, Run, Seat, Selection, Session};

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(Net::from_env())
        .init_resource::<client::Replicas>()
        .init_resource::<host::Replicator>()
        .init_resource::<Party>()
        .add_systems(PreUpdate, pump)
        .add_systems(Last, flush)
        .add_systems(Update, start_session.before(super::sync::begin_run))
        .add_systems(OnEnter(Phase::Menu), left_run)
        .add_systems(OnEnter(Phase::Lobby), back_to_lobby);
    host::plugin(app);
    client::plugin(app);
    ui::plugin(app);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Status {
    Offline,
    Connecting,
    /// Connected and greeted: lobby requests go straight out.
    Online,
}

/// The lobby connection and what the server last said.
#[derive(Resource)]
pub(super) struct Net {
    /// The lobby's WebSocket URL.
    pub server: String,
    pub name: String,
    socket: Option<Socket>,
    pub status: Status,
    /// The lobby this machine sits in.
    pub lobby: Option<LobbyView>,
    /// The server's listed lobbies, from the last browse.
    pub lobbies: Vec<LobbySummary>,
    /// A line for the lobby screen: an error, or why a lobby closed.
    pub notice: Option<String>,
    /// Game messages relayed to this machine, by sender seat.
    pub inbox: Vec<(u8, Game)>,
    /// Requests made before the server greeted us.
    queued: Vec<Up>,
    /// A run's seed, from [`Down::Started`] until [`start_session`] takes it.
    started: Option<u64>,
    /// Relay frames this frame produced, sent once at its end ([`flush`]).
    pub outbox: Vec<Up>,
}

impl Net {
    fn from_env() -> Self {
        let server = crate::webenv::var("WOW_LOBBY").unwrap_or_else(default_server);
        let name = crate::webenv::var("WOW_NAME")
            .map(|n| clean_name(&n, MAX_PLAYER_NAME))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Adventurer".to_string());
        Self {
            server,
            name,
            socket: None,
            status: Status::Offline,
            lobby: None,
            lobbies: Vec::new(),
            notice: None,
            inbox: Vec::new(),
            queued: Vec::new(),
            started: None,
            outbox: Vec::new(),
        }
    }

    /// Open the connection if it is not; requests wait for the greeting.
    pub fn connect(&mut self) {
        if self.status != Status::Offline {
            return;
        }
        self.socket = Some(Socket::connect(&self.server));
        self.status = Status::Connecting;
        self.notice = None;
    }

    pub fn disconnect(&mut self) {
        self.socket = None;
        self.status = Status::Offline;
        self.lobby = None;
        self.queued.clear();
        self.inbox.clear();
        self.outbox.clear();
        self.started = None;
    }

    pub fn send(&mut self, up: Up) {
        match (self.status, &self.socket) {
            (Status::Online, Some(socket)) => socket.send(up.encode()),
            _ => {
                self.queued.push(up);
                self.connect();
            }
        }
    }

    /// A tick's game messages to the run's other seats, sent at the frame's end.
    pub fn relay(&mut self, to: Target, msgs: &[Game]) {
        if msgs.is_empty() {
            return;
        }
        let data = encode_batch(msgs);
        self.outbox.push(Up::Relay { to, data });
    }
}

/// The page's own origin, where `wenilla-host` mounts the lobby; on the desktop, a lobby server on
/// this machine.
fn default_server() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(location) = web_sys::window().map(|w| w.location()) {
            if let (Ok(protocol), Ok(host)) = (location.protocol(), location.host()) {
                let scheme = if protocol == "https:" { "wss" } else { "ws" };
                return format!("{scheme}://{host}{LOBBY_PATH}");
            }
        }
    }
    format!("ws://127.0.0.1:8090{LOBBY_PATH}")
}

/// The local pick as the lobby shows it.
pub(super) fn loadout(selection: &Selection) -> Loadout {
    let (display, race, sex) = selection
        .look
        .and_then(|i| selection.looks.get(i))
        .copied()
        .unwrap_or((0, 0, 0));
    Loadout {
        class: selection.class.unwrap_or(0) as u8,
        display,
        race,
        sex,
    }
}

/// A run's rules from its lobby's settings.
pub(super) fn rules_for(settings: &Settings, heroes: usize) -> Rules {
    let s = settings.sanitized();
    let pct = |v: u16| f32::from(v) / 100.0;
    Rules {
        duration: f32::from(s.duration_secs),
        boss_every: f32::from(s.boss_every_secs),
        health: pct(s.difficulty.enemy_health),
        damage: pct(s.difficulty.enemy_damage),
        density: pct(s.difficulty.density),
        scale_with_heroes: s.difficulty.scale_with_players,
        heroes: heroes.max(1) as u32,
        shared_xp: s.shared_xp,
        revive: s.revive,
    }
}

/// The other heroes, for the party frames and the results.
#[derive(Resource, Default)]
pub(super) struct Party {
    pub members: Vec<PartyMember>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PartyMember {
    pub slot: u8,
    pub name: String,
    pub class: usize,
    pub level: u32,
    pub hp: f32,
    pub max_hp: f32,
    pub dead: bool,
    pub kills: u32,
    pub damage_done: f64,
}

/// Read the socket: greet, then file every answer.
#[allow(clippy::too_many_arguments)]
fn pump(
    mut net: ResMut<Net>,
    mut session: ResMut<Session>,
    phase: Res<State<Phase>>,
    mut next: ResMut<NextState<Phase>>,
) {
    let Some(socket) = &net.socket else {
        return;
    };
    let events = socket.poll();
    for ev in events {
        match ev {
            SocketEvent::Open => {
                let name = net.name.clone();
                if let Some(s) = &net.socket {
                    s.send(
                        Up::Hello {
                            version: PROTOCOL_VERSION,
                            name,
                        }
                        .encode(),
                    );
                }
            }
            SocketEvent::Frame(bytes) => {
                let Ok(down) = Down::decode(&bytes) else {
                    continue;
                };
                handle(down, &mut net, &mut session, *phase.get(), &mut next);
            }
            SocketEvent::Closed(why) => {
                let was_in = net.lobby.is_some();
                net.disconnect();
                net.notice = Some(if was_in {
                    format!("Disconnected from the lobby: {why}")
                } else {
                    why
                });
                drop_out(&mut session, *phase.get(), &mut next);
                break;
            }
        }
    }
    session.in_lobby = net.lobby.is_some();
}

/// The frame's relay frames go out; with no connection they are dropped.
fn flush(mut net: ResMut<Net>) {
    let out = std::mem::take(&mut net.outbox);
    if net.status != Status::Online {
        return;
    }
    if let Some(socket) = &net.socket {
        for up in out {
            socket.send(up.encode());
        }
    }
}

/// Out of the lobby: a multiplayer run ends here, and the menu takes over.
fn drop_out(session: &mut Session, phase: Phase, next: &mut NextState<Phase>) {
    let was_multi = session.multiplayer();
    *session = Session::default();
    if phase == Phase::Lobby || (was_multi && phase != Phase::Menu) {
        next.set(Phase::Menu);
    }
}

fn handle(
    down: Down,
    net: &mut Net,
    session: &mut Session,
    phase: Phase,
    next: &mut NextState<Phase>,
) {
    match down {
        Down::Welcome { .. } => {
            net.status = Status::Online;
            let queued: Vec<Up> = std::mem::take(&mut net.queued);
            if let Some(s) = &net.socket {
                for up in queued {
                    s.send(up.encode());
                }
            }
        }
        Down::Lobby(view) => {
            let first = net.lobby.is_none();
            net.lobby = Some(view);
            if first && phase == Phase::Menu {
                next.set(Phase::Lobby);
            }
        }
        Down::Lobbies(list) => net.lobbies = list,
        Down::Started { seed } => {
            // Whatever came before belongs to no run; what follows in this same read is the new
            // run's first stream, and must survive until the run begins.
            net.inbox.clear();
            net.started = Some(seed);
        }
        Down::Ended => {
            if matches!(
                phase,
                Phase::Playing | Phase::LevelUp | Phase::Paused | Phase::GameOver
            ) {
                next.set(Phase::Lobby);
            }
        }
        Down::Relay { from, data } => {
            for g in decode_batch(&data) {
                net.inbox.push((from, g));
            }
            // A guest that stopped reading must not grow without bound.
            if net.inbox.len() > 20_000 {
                let excess = net.inbox.len() - 20_000;
                net.inbox.drain(..excess);
            }
        }
        Down::Left(reason) => {
            net.lobby = None;
            net.notice = match reason {
                LeftReason::Left => None,
                LeftReason::Kicked => Some("The host removed you from the lobby.".into()),
                LeftReason::HostLeft => Some("The host left; the lobby is closed.".into()),
            };
            drop_out(session, phase, next);
        }
        Down::Error(code) => net.notice = Some(code.message().to_string()),
        Down::Pong(_) => {}
    }
}

/// A run starts for the whole table: the mode, the rules, the seats, the zone, the seed.
#[allow(clippy::too_many_arguments)]
fn start_session(
    mut commands: Commands,
    mut net: ResMut<Net>,
    mut session: ResMut<Session>,
    mut run: ResMut<Run>,
    mut selection: ResMut<Selection>,
    mut rng: ResMut<Rng>,
    mut next: ResMut<NextState<Phase>>,
    mut reps: ResMut<client::Replicas>,
    mut rep: ResMut<host::Replicator>,
    mut party: ResMut<Party>,
    local: Query<Entity, With<super::LocalHero>>,
) {
    let Some(seed) = net.started.take() else {
        return;
    };
    reps.reset();
    rep.reset();
    party.members.clear();
    let Some(view) = net.lobby.clone() else {
        return;
    };
    let me = view.me().cloned();
    session.mode = if view.is_host() {
        Mode::Host
    } else {
        Mode::Client
    };
    session.slot = view.you;
    session.rules = rules_for(&view.settings, view.members.len());
    session.seats = view
        .members
        .iter()
        .map(|m| Seat {
            slot: m.slot,
            name: m.name.clone(),
            class: usize::from(m.loadout.class).min(data::CLASSES.len() - 1),
            display: m.loadout.display,
            race: m.loadout.race,
            sex: m.loadout.sex,
        })
        .collect();
    session.in_lobby = true;
    *rng = Rng::seeded(seed ^ u64::from(view.you).wrapping_mul(0x9E37_79B9_7F4A_7C15));

    // The run's zone, and this player's hero as the lobby knows it.
    selection.map = usize::from(view.settings.zone).min(data::MAPS.len() - 1);
    if let Some(m) = &me {
        let class = usize::from(m.loadout.class).min(data::CLASSES.len() - 1);
        if selection.class != Some(class) {
            selection.class = Some(class);
        }
        let look = (m.loadout.display, m.loadout.race, m.loadout.sex);
        if !selection.looks.contains(&look) {
            selection.looks.push(look);
        }
        selection.look = selection.looks.iter().position(|l| *l == look);
    }
    if !super::hud::start_run(&mut run, &mut selection) {
        // No look known for this class yet: start from the loadout's own numbers.
        let class = &data::CLASSES[selection.class.unwrap_or(0).min(data::CLASSES.len() - 1)];
        let next_guid = run.next_guid;
        *run = Run {
            me: super::Vitals::new(class, me.as_ref().map_or(0, |m| m.loadout.race)),
            next_guid,
            ..Run::default()
        };
        selection.stage = selection.map;
    }
    // Every seat must load the zone before the clock runs.
    run.waiting = view.members.len() > 1;
    match session.mode {
        Mode::Host => {
            commands.insert_resource(super::units::FxLog::default());
            // Guids are the host's: start them clear of anything a guest may have minted.
            run.next_guid = run.next_guid.max(1 << 20);
        }
        Mode::Client => {
            commands.remove_resource::<super::units::FxLog>();
            // The guest's hero comes from the host; the menu's preview goes.
            for e in &local {
                commands.entity(e).despawn();
            }
        }
        Mode::Solo => {}
    }
    next.set(Phase::Playing);
}

/// Back at the menu from a multiplayer run (the pause menu's "Main menu"): leave the lobby.
fn left_run(mut net: ResMut<Net>, mut session: ResMut<Session>, mut commands: Commands) {
    commands.remove_resource::<super::units::FxLog>();
    if session.multiplayer() {
        if net.lobby.is_some() {
            net.send(Up::Leave);
            net.lobby = None;
        }
        *session = Session::default();
    }
    session.in_lobby = net.lobby.is_some();
}

/// Back at the lobby after a run (the host reopens it for everyone), or from the menu with a new
/// hero (the table sees it).
fn back_to_lobby(
    mut net: ResMut<Net>,
    mut session: ResMut<Session>,
    selection: Res<Selection>,
    mut commands: Commands,
) {
    commands.remove_resource::<super::units::FxLog>();
    if net.lobby.is_some() && selection.class.is_some() {
        net.send(Up::SetLoadout(loadout(&selection)));
    }
    let host_in_run = session.mode == Mode::Host
        && net
            .lobby
            .as_ref()
            .is_some_and(|l| l.state == survivors_net::protocol::LobbyState::InRun);
    if host_in_run {
        net.send(Up::EndRun);
    }
    // Between runs the table plays nothing.
    let keep_lobby = session.in_lobby;
    *session = Session::default();
    session.in_lobby = keep_lobby;
}

#[cfg(test)]
mod tests {
    use super::*;
    use survivors_net::settings::{Difficulty, ZONE_COUNT};

    use crate::creature_anim::{CastEvent, MovementState, PlaySeq, SpellGoTargets, SwingMessage};
    use crate::net::{FieldChanged, Guid, NetEntity, ObjectStore};
    use crate::survivors::units::{FxEvent, FxLog};
    use crate::survivors::{HeroState, Hostile, LocalHero, RunEntity, Vitals};
    use benilla_protocol::messages::ObjectFields;

    /// A world with what the stream and its copy need, and nothing of the engine.
    fn world(mode: Mode, slot: u8) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Net::from_env())
            .init_resource::<client::Replicas>()
            .init_resource::<host::Replicator>()
            .init_resource::<Party>()
            .init_resource::<PlaySeq>()
            .add_message::<CastEvent>()
            .add_message::<SpellGoTargets>()
            .add_message::<SwingMessage>()
            .add_message::<crate::combat_text::CombatTextSpawn>()
            .add_message::<FieldChanged>()
            .insert_resource(Session {
                mode,
                slot,
                seats: vec![
                    Seat {
                        slot: 0,
                        name: "Host".into(),
                        class: 0,
                        display: 1,
                        race: 1,
                        sex: 0,
                    },
                    Seat {
                        slot: 1,
                        name: "Guest".into(),
                        class: 7,
                        display: 2,
                        race: 2,
                        sex: 1,
                    },
                ],
                ..Session::default()
            })
            .insert_resource(Run {
                begun: true,
                ..Run::default()
            });
        app
    }

    fn unit(
        guid: u64,
        health: u32,
    ) -> (
        Guid,
        NetEntity,
        ObjectStore,
        Transform,
        RunEntity,
        MovementState,
    ) {
        (
            Guid(guid),
            NetEntity {
                kind: benilla_protocol::EntityKind::Unit,
                display_id: Some(1234),
                scale: 1.0,
            },
            ObjectStore(ObjectFields::from_pairs(&[
                (0, guid as u32),
                (1, (guid >> 32) as u32),
                (2, 0x9),
                (22, health),
                (28, 100),
            ])),
            Transform::from_xyz(1.0, 0.0, 2.0),
            RunEntity,
            MovementState::default(),
        )
    }

    /// Every relay frame the host produced, delivered to the guest in seat 1.
    fn deliver(host: &mut App, guest: &mut App) -> usize {
        let frames = std::mem::take(&mut host.world_mut().resource_mut::<Net>().outbox);
        let mut n = 0;
        for up in frames {
            let Up::Relay { to, data } = up else { continue };
            if matches!(to, Target::Others | Target::Slot(1)) {
                let msgs = decode_batch(&data);
                n += msgs.len();
                guest
                    .world_mut()
                    .resource_mut::<Net>()
                    .inbox
                    .extend(msgs.into_iter().map(|m| (0, m)));
            }
        }
        n
    }

    fn by_guid(app: &mut App, guid: u64) -> Option<Entity> {
        let mut q = app.world_mut().query::<(Entity, &Guid)>();
        q.iter(app.world())
            .find(|(_, g)| g.0 == guid)
            .map(|(e, _)| e)
    }

    #[test]
    fn the_hosts_run_is_rebuilt_on_the_guest() {
        let mut host = world(Mode::Host, 0);
        host.add_systems(Update, host::stream);
        host.insert_resource(FxLog::default());
        let mut guest = world(Mode::Client, 1);
        guest.add_systems(Update, client::receive);

        // The host's world: an enemy, and the guest's hero.
        let enemy = host.world_mut().spawn((unit(100, 100), Hostile)).id();
        let class = &data::CLASSES[7];
        let mut v = Vitals::new(class, 2);
        v.level = 6;
        let hero = host
            .world_mut()
            .spawn((unit(200, 100), HeroState { slot: 1, v }))
            .id();
        host.world_mut()
            .resource_mut::<FxLog>()
            .0
            .push(FxEvent::Cast {
                caster: hero,
                spell: 133,
                hits: vec![enemy],
                dest: None,
            });
        host.world_mut().resource_mut::<host::Replicator>().due();
        host.update();
        assert!(deliver(&mut host, &mut guest) > 0);
        guest.update();

        // Both units exist on the guest, the enemy hostile, the hero its own.
        let copy = by_guid(&mut guest, 100).expect("the enemy is copied");
        assert!(guest.world().get::<Hostile>(copy).is_some());
        let mine = by_guid(&mut guest, 200).expect("the guest's hero is copied");
        assert!(guest.world().get::<LocalHero>(mine).is_some());
        // The cast played on the guest.
        let casts = guest.world().resource::<Messages<CastEvent>>();
        assert!(casts
            .iter_current_update_messages()
            .any(|c| c.spell_id == 133 && c.entity == mine));
        // The guest's view took the host's numbers for its hero.
        assert_eq!(guest.world().resource::<Run>().level, 6);
        assert_eq!(guest.world().resource::<Session>().name_of(1), "Guest");

        // The enemy dies and moves on the host: the guest sees both.
        {
            let mut store = host.world_mut().get_mut::<ObjectStore>(enemy).unwrap();
            crate::survivors::units::kill_unit(&mut store);
        }
        host.world_mut()
            .get_mut::<Transform>(enemy)
            .unwrap()
            .translation = Vec3::new(5.0, 0.0, 5.0);
        host.world_mut().resource_mut::<host::Replicator>().due();
        host.update();
        deliver(&mut host, &mut guest);
        guest.update();
        let store = guest.world().get::<ObjectStore>(copy).unwrap();
        assert!(
            crate::survivors::units::is_dead(store),
            "the death reached the copy"
        );
        let edges = guest.world().resource::<Messages<FieldChanged>>();
        assert!(edges
            .iter_current_update_messages()
            .any(|e| e.entity == copy && e.index == 22 && e.new == 0));

        // The enemy goes on the host: its copy goes on the guest.
        host.world_mut().despawn(enemy);
        host.world_mut().resource_mut::<host::Replicator>().due();
        host.update();
        deliver(&mut host, &mut guest);
        guest.update();
        assert!(
            by_guid(&mut guest, 100).is_none(),
            "the despawn reached the guest"
        );
        assert!(by_guid(&mut guest, 200).is_some());
    }

    #[test]
    fn the_wire_knows_every_zone() {
        assert_eq!(usize::from(ZONE_COUNT), data::MAPS.len());
    }

    #[test]
    fn lobby_settings_become_rules() {
        let settings = Settings {
            duration_secs: 600,
            boss_every_secs: 90,
            difficulty: Difficulty::HARD,
            shared_xp: false,
            revive: true,
            ..Settings::default()
        };
        let r = rules_for(&settings, 3);
        assert_eq!(r.duration, 600.0);
        assert_eq!(r.boss_every, 90.0);
        assert_eq!(r.health, 1.5);
        assert_eq!(r.heroes, 3);
        assert!(!r.shared_xp && r.revive);
        assert_eq!(
            rules_for(&Settings::default(), 1).duration,
            data::VICTORY_SECS
        );
    }
}
