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
mod smooth;
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
        appearance: selection.appearance,
        bonuses: selection.brings.bonuses.clone(),
        gear: selection.brings.gear.clone(),
        perks: selection.brings.perks.clone(),
        tabard: selection.brings.tabard.clone(),
    }
}

/// A run's rules from its lobby's settings.
pub(super) fn rules_for(settings: &Settings, heroes: usize) -> Rules {
    let s = settings.sanitized();
    let pct = |v: u16| f32::from(v) / 100.0;
    Rules {
        duration: f32::from(s.duration_secs),
        ambush_every: f32::from(s.ambush_every_secs),
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

/// Most messages a machine holds unread before it sheds some.
const MAX_INBOX: usize = 20_000;

/// Whether a message can be shed when the inbox is full: what only shows (a cast, a number, a
/// swing, an impact) and motion, which the next motion supersedes. A unit's coming, its fields,
/// its going, the run's state and a hero's numbers are never shed: losing one would leave the
/// copy wrong for the rest of the run.
fn sheddable(g: &Game) -> bool {
    matches!(
        g,
        Game::Cast { .. }
            | Game::Impact { .. }
            | Game::Swing { .. }
            | Game::Number { .. }
            | Game::Motions { .. }
            | Game::Input { .. }
    )
}

/// Hold `inbox` to `cap`: the oldest sheddable messages go first, and only those.
fn trim_inbox(inbox: &mut Vec<(u8, Game)>, cap: usize) {
    let over = inbox.len().saturating_sub(cap);
    if over == 0 {
        return;
    }
    let mut shed = 0;
    inbox.retain(|(_, g)| {
        if shed < over && sheddable(g) {
            shed += 1;
            false
        } else {
            true
        }
    });
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
            trim_inbox(&mut net.inbox, MAX_INBOX);
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
            appearance: m.loadout.appearance,
            brings: super::Brings {
                bonuses: m.loadout.bonuses.clone(),
                gear: m.loadout.gear.clone(),
                perks: m.loadout.perks.clone(),
                tabard: m.loadout.tabard.clone(),
            },
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
    use crate::survivors::{HeroState, Hostile, LocalHero, Pickup, RunEntity, Vitals};
    use benilla_protocol::messages::ObjectFields;

    /// A world with what the stream and its copy need, and nothing of the engine.
    fn world(mode: Mode, slot: u8) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(Net::from_env())
            .init_resource::<client::Replicas>()
            .init_resource::<host::Replicator>()
            .init_resource::<crate::survivors::market::BuyQueue>()
            .init_resource::<crate::survivors::Selection>()
            .init_resource::<Party>()
            .init_resource::<PlaySeq>()
            .add_message::<CastEvent>()
            .add_message::<SpellGoTargets>()
            .add_message::<SwingMessage>()
            .add_message::<crate::combat_text::CombatTextSpawn>()
            .add_message::<crate::survivors::feel::Struck>()
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
                        appearance: [0; 5],
                        brings: Default::default(),
                    },
                    Seat {
                        slot: 1,
                        name: "Guest".into(),
                        class: 7,
                        display: 2,
                        race: 2,
                        sex: 1,
                        appearance: [0; 5],
                        brings: Default::default(),
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
    fn the_field_reaches_the_guest() {
        use crate::survivors::treasure::{FieldMark, PostView};
        let mut host = world(Mode::Host, 0);
        host.add_systems(Update, host::stream);
        let mut guest = world(Mode::Client, 1);
        guest.add_systems(Update, client::receive);

        // An elite at its post, a chest, and an empty post coming back.
        host.world_mut()
            .spawn((unit(300, 100), Hostile, FieldMark::Elite(2)));
        host.world_mut()
            .spawn((unit(301, 100), Pickup, FieldMark::Chest));
        host.world_mut().resource_mut::<Run>().posts = vec![PostView {
            kind: 1,
            pos: Vec3::new(4.0, 0.0, -8.0),
            wait: 30.0,
        }];
        host.world_mut().resource_mut::<Run>().chest_cost = 55;
        host.world_mut().resource_mut::<host::Replicator>().due();
        host.update();
        deliver(&mut host, &mut guest);
        guest.update();

        let elite = by_guid(&mut guest, 300).expect("the elite is copied");
        assert_eq!(
            guest.world().get::<FieldMark>(elite),
            Some(&FieldMark::Elite(2))
        );
        assert!(guest.world().get::<Hostile>(elite).is_some());
        let chest = by_guid(&mut guest, 301).expect("the chest is copied");
        assert_eq!(
            guest.world().get::<FieldMark>(chest),
            Some(&FieldMark::Chest)
        );
        let run = guest.world().resource::<Run>();
        assert_eq!(run.chest_cost, 55);
        assert_eq!(run.posts.len(), 1);
        assert_eq!(run.posts[0].kind, 1);
        assert!((run.posts[0].wait - 30.0).abs() < 1e-4);
        assert_eq!(run.posts[0].pos, Vec3::new(4.0, 0.0, -8.0));
    }

    #[test]
    fn the_host_deals_a_guests_cards_and_checks_its_moves() {
        use crate::survivors::{offer, Rng, Selection};
        use survivors_net::game::{Card as WireCard, CardAction};
        let mut host = world(Mode::Host, 0);
        host.init_resource::<Rng>()
            .init_resource::<Selection>()
            .init_resource::<crate::loading_screen::LoadingScreen>()
            .init_resource::<crate::survivors::mobility::MobilityQueue>()
            .add_systems(Update, host::receive)
            .add_systems(PostUpdate, (offer::deal_offers, host::stream).chain());
        let mut guest = world(Mode::Client, 1);
        guest.add_systems(Update, client::receive);

        // The guest's hero has a level banked: the host deals it three cards.
        let mut v = Vitals::new(&data::CLASSES[7], 2);
        v.pending_levels = 1;
        let hero = host
            .world_mut()
            .spawn((
                unit(400, 100),
                HeroState { slot: 1, v },
                crate::survivors::hero::Hero::default(),
            ))
            .id();
        host.world_mut().resource_mut::<host::Replicator>().due();
        host.update();
        deliver(&mut host, &mut guest);
        guest.update();
        let dealt = host.world().get::<HeroState>(hero).unwrap().offer.clone();
        assert_eq!(dealt.len(), data::CARDS_DEALT);
        let seen = guest.world().resource::<Run>().offer.clone();
        assert_eq!(seen, dealt, "the guest sees the cards the host dealt");

        // A card claimed at a rarity it was not dealt at is refused.
        let mut forged = dealt[0].to_wire();
        forged.rarity = (forged.rarity + 1) % 5;
        let never = WireCard {
            choice: survivors_net::game::Choice::NewAbility("no_such_spell".into()),
            rarity: 0,
            perks: Vec::new(),
        };
        for bad in [forged, never] {
            host.world_mut()
                .resource_mut::<Net>()
                .inbox
                .push((1, Game::Card(CardAction::Pick(bad))));
        }
        host.update();
        let h = host.world().get::<HeroState>(hero).unwrap();
        assert_eq!(h.pending_levels, 1);
        assert_eq!(h.picks, 0);

        // The card as dealt lands.
        host.world_mut()
            .resource_mut::<Net>()
            .inbox
            .push((1, Game::Card(CardAction::Pick(dealt[0].to_wire()))));
        host.update();
        let h = host.world().get::<HeroState>(hero).unwrap();
        assert_eq!(h.pending_levels, 0);
        assert_eq!(h.picks, 1);
        assert!(h.offer.is_empty(), "no level left, no cards");
    }

    #[test]
    fn a_guests_blessing_reaches_its_view_and_the_host_checks_it() {
        use crate::survivors::{treasure, Rng, Selection};
        let mut host = world(Mode::Host, 0);
        host.init_resource::<Rng>()
            .init_resource::<Selection>()
            .init_resource::<crate::loading_screen::LoadingScreen>()
            .init_resource::<crate::survivors::mobility::MobilityQueue>()
            .add_systems(Update, host::receive)
            .add_systems(PostUpdate, host::stream);
        let mut guest = world(Mode::Client, 1);
        guest.add_systems(Update, client::receive);

        // A Shrine of Power charged under the guest's hero: the host offers it three blessings.
        let mut v = Vitals::new(&data::CLASSES[7], 2);
        v.shrine_offer = treasure::deal_boons(&mut Rng::seeded(3));
        let hero = host
            .world_mut()
            .spawn((
                unit(500, 100),
                HeroState { slot: 1, v },
                crate::survivors::hero::Hero::default(),
            ))
            .id();
        host.world_mut().resource_mut::<host::Replicator>().due();
        host.update();
        deliver(&mut host, &mut guest);
        guest.update();
        let offered = host
            .world()
            .get::<HeroState>(hero)
            .unwrap()
            .shrine_offer
            .clone();
        assert_eq!(guest.world().resource::<Run>().shrine_offer, offered);

        // A blessing never offered is refused; one of the offer stays for the run.
        let forged = data::bonus_to_wire(&data::Bonus::flat(data::Stat::Might, 5.0));
        let real = data::bonus_to_wire(&offered[2].bonus);
        for b in [forged, real] {
            host.world_mut()
                .resource_mut::<Net>()
                .inbox
                .push((1, Game::ShrineChoice(b)));
        }
        host.update();
        let h = host.world().get::<HeroState>(hero).unwrap();
        assert_eq!(h.boons, [offered[2].bonus]);
        assert!(h.shrine_offer.is_empty());
    }

    #[test]
    fn a_guests_purchase_goes_to_the_market_by_its_seat() {
        use crate::survivors::{market, Rng, Selection};
        let mut host = world(Mode::Host, 0);
        host.init_resource::<Rng>()
            .init_resource::<Selection>()
            .init_resource::<crate::loading_screen::LoadingScreen>()
            .init_resource::<crate::survivors::mobility::MobilityQueue>()
            .add_systems(Update, host::receive);
        host.world_mut().resource_mut::<Net>().inbox.push((
            1,
            Game::Buy {
                slot: 2,
                item: "lodestone".into(),
            },
        ));
        host.update();
        // The market takes it from here, for the hero in seat 1 only.
        assert_eq!(
            host.world().resource::<market::BuyQueue>().0,
            [(1, 2, "lodestone".to_string())]
        );
    }

    #[test]
    fn a_guest_follows_the_host_to_the_next_stage() {
        use crate::survivors::{hero, Selection};
        let mut guest = world(Mode::Client, 1);
        guest.add_systems(Update, (client::receive, client::arrive).chain());
        let me = guest
            .world_mut()
            .spawn((Transform::default(), LocalHero))
            .id();
        let run_at = |stage: u8, zone: u8| Game::Run {
            elapsed: 4.0,
            bosses: 0,
            waiting: true,
            banner: None,
            chest_cost: 20,
            posts: Vec::new(),
            shrines: Vec::new(),
            greed: 0,
            curse: 0,
            shop: Vec::new(),
            stage,
            zone,
            gates: vec![survivors_net::game::Gate {
                kind: 0,
                pos: [1.0, 2.0, 3.0],
                charge: 0.0,
                open: false,
            }],
            swarm: -1.0,
        };
        guest
            .world_mut()
            .resource_mut::<Net>()
            .inbox
            .push((0, run_at(1, 3)));
        guest.update();
        let world = guest.world();
        assert_eq!(
            world.resource::<Selection>().stage,
            3,
            "the world follows the zone"
        );
        let run = world.resource::<Run>();
        assert_eq!(run.leg, 1);
        assert_eq!(run.gates.len(), 1);
        assert!(run.swarm.is_none());
        let at = world.get::<Transform>(me).unwrap().translation;
        let center = hero::arena(&data::MAPS[3]);
        assert!(at.distance(center) < 5.0, "the hero went over to it");
    }

    /// xorshift64*, for the link's whims: the same on every run.
    struct Whims(u64);
    impl Whims {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x >> 12;
            x ^= x << 25;
            x ^= x >> 27;
            self.0 = x;
            x.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
    }

    /// The host's relay frames for the guest in seat 1, as bytes.
    fn frames_for_guest(host: &mut App) -> Vec<Vec<u8>> {
        std::mem::take(&mut host.world_mut().resource_mut::<Net>().outbox)
            .into_iter()
            .filter_map(|up| match up {
                Up::Relay { to, data } if matches!(to, Target::Others | Target::Slot(1)) => {
                    Some(data)
                }
                _ => None,
            })
            .collect()
    }

    /// A frame reaches the guest as the server would hand it on: decoded, from its sender.
    fn arrive(guest: &mut App, from: u8, data: &[u8]) {
        let msgs = decode_batch(data);
        guest
            .world_mut()
            .resource_mut::<Net>()
            .inbox
            .extend(msgs.into_iter().map(|m| (from, m)));
    }

    /// The host's world over a bad link: frames late by up to 400 ms and in bursts, as a slow
    /// network delivers TCP (in order, never lost); a guest's frames that only the host may send;
    /// a host frame the transport cut; NaN positions in well-formed messages. Units come, move,
    /// are hurt and go all the while. Once the link has caught up, the guest's copy is the host's:
    /// the same units, at the same places, with the same fields, and none of the poison.
    #[test]
    fn the_guest_converges_through_a_bad_link() {
        use survivors_net::game::{encode_batch, Motion, ObjectKind, Role, Spawn};
        use std::collections::{HashMap, VecDeque};
        for seed in 1..=6u64 {
            let mut whims = Whims(0xBAD_11E5 ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
            let mut host = world(Mode::Host, 0);
            host.add_systems(Update, host::stream);
            host.insert_resource(FxLog::default());
            let mut guest = world(Mode::Client, 1);
            guest.add_systems(Update, client::receive);

            let mut alive: Vec<(Entity, u64)> = Vec::new();
            let mut next_guid = 1000u64;
            // In order, as TCP: a frame is never handed on before the one sent before it.
            let mut link: VecDeque<(u32, Vec<u8>)> = VecDeque::new();
            let mut last_due = 0u32;
            for tick in 0..420u32 {
                let churn = tick < 380;
                if churn {
                    // Units come and go.
                    for _ in 0..whims.below(3) {
                        let e = host.world_mut().spawn((unit(next_guid, 100), Hostile)).id();
                        alive.push((e, next_guid));
                        next_guid += 1;
                    }
                    if !alive.is_empty() && whims.below(3) == 0 {
                        let (e, _) = alive.swap_remove(whims.below(alive.len() as u64) as usize);
                        host.world_mut().despawn(e);
                    }
                    // They move and are hurt.
                    for &(e, _) in &alive {
                        if whims.below(2) == 0 {
                            let step = Vec3::new(
                                whims.below(100) as f32 / 100.0 - 0.5,
                                0.0,
                                whims.below(100) as f32 / 100.0 - 0.5,
                            );
                            host.world_mut().get_mut::<Transform>(e).unwrap().translation += step;
                        }
                        if whims.below(6) == 0 {
                            let hp = 1 + whims.below(100) as u32;
                            host.world_mut()
                                .get_mut::<ObjectStore>(e)
                                .unwrap()
                                .0
                                .merge(ObjectFields::from_pairs(&[(22, hp)]));
                        }
                    }
                }
                host.world_mut().resource_mut::<Run>().elapsed += host::TICK;
                host.world_mut().resource_mut::<host::Replicator>().due();
                host.update();

                for data in frames_for_guest(&mut host) {
                    // Late by up to eight ticks, never ahead of what went before.
                    let due = (tick + whims.below(9) as u32).max(last_due);
                    last_due = due;
                    link.push_back((due, data));
                }
                // Poison, now and then, mixed into the stream.
                if churn && whims.below(10) == 0 {
                    let t = host.world().resource::<Run>().elapsed;
                    let victim = alive.first().map_or(1, |a| a.1);
                    let bad = encode_batch(&[
                        Game::Motions {
                            t,
                            list: vec![Motion {
                                guid: victim,
                                pos: [f32::NAN, 0.0, f32::INFINITY],
                                yaw: f32::NAN,
                                speed: 1.0,
                                flags: 0,
                                vertical_speed: 0.0,
                            }],
                        },
                        Game::Spawn(Spawn {
                            guid: 9_999_999,
                            kind: ObjectKind::Unit,
                            role: Role::Enemy,
                            display: Some(1),
                            scale: 1.0,
                            fields: vec![],
                            pos: [f32::NAN; 3],
                            yaw: 0.0,
                        }),
                    ]);
                    link.push_back((last_due, bad));
                }
                // The guest goes through its inbox each frame, whether or not anything came.
                while link.front().is_some_and(|(due, _)| *due <= tick) {
                    let (_, data) = link.pop_front().unwrap();
                    arrive(&mut guest, 0, &data);
                    // A frame cut short by a broken transport, and another guest's word on the
                    // run, which only the host may give.
                    if whims.below(25) == 0 {
                        arrive(&mut guest, 0, &data[..data.len() / 2]);
                    }
                    if whims.below(25) == 0 {
                        let spawn = encode_batch(&[Game::Despawn {
                            guid: alive.first().map_or(1, |a| a.1),
                            fade: false,
                        }]);
                        arrive(&mut guest, 2, &spawn);
                    }
                }
                guest.update();
            }
            // The link catches up.
            while let Some((_, data)) = link.pop_front() {
                arrive(&mut guest, 0, &data);
            }
            guest.update();

            let want: HashMap<u64, (Vec3, u32)> = alive
                .iter()
                .map(|&(e, g)| {
                    let w = host.world();
                    let hp = w
                        .get::<ObjectStore>(e)
                        .unwrap()
                        .0
                        .raw_fields()
                        .find(|(i, _)| *i == 22)
                        .map_or(0, |(_, v)| v);
                    (g, (w.get::<Transform>(e).unwrap().translation, hp))
                })
                .collect();
            let mut q = guest
                .world_mut()
                .query::<(&Guid, &ObjectStore, Option<&client::Replica>)>();
            let got: HashMap<u64, (Option<Vec3>, u32)> = q
                .iter(guest.world())
                .map(|(g, store, rep)| {
                    let hp = store
                        .0
                        .raw_fields()
                        .find(|(i, _)| *i == 22)
                        .map_or(0, |(_, v)| v);
                    (g.0, (rep.and_then(|r| r.newest()), hp))
                })
                .collect();
            assert!(want.len() > 20, "seed {seed}: the field filled up");
            assert_eq!(
                got.keys().collect::<std::collections::BTreeSet<_>>(),
                want.keys().collect::<std::collections::BTreeSet<_>>(),
                "seed {seed}: the guest has the host's units, and no poisoned one"
            );
            for (g, (pos, hp)) in &want {
                let (copy, copy_hp) = got[g];
                let copy = copy.expect("a copy is drawn from the host's track");
                assert!(
                    copy.is_finite() && copy.distance(*pos) < 0.05,
                    "seed {seed}: unit {g} at {copy} on the guest, {pos} on the host"
                );
                assert_eq!(copy_hp, *hp, "seed {seed}: unit {g}'s health");
            }
        }
    }

    #[test]
    fn a_full_inbox_sheds_only_what_the_next_frame_restores() {
        use survivors_net::game::Spawn;
        let spawn = |g| {
            Game::Spawn(Spawn {
                guid: g,
                kind: survivors_net::game::ObjectKind::Unit,
                role: survivors_net::game::Role::Enemy,
                display: None,
                scale: 1.0,
                fields: vec![],
                pos: [0.0; 3],
                yaw: 0.0,
            })
        };
        let mut inbox: Vec<(u8, Game)> = Vec::new();
        for g in 0..50u64 {
            inbox.push((0, spawn(g)));
            inbox.push((
                0,
                Game::Number {
                    anchor: g,
                    amount: 1.0,
                    crit: false,
                    color: 0,
                },
            ));
            inbox.push((0, Game::Motions { t: g as f32, list: vec![] }));
            inbox.push((0, Game::Despawn { guid: g, fade: false }));
        }
        trim_inbox(&mut inbox, 120);
        assert_eq!(inbox.len(), 120);
        // Every spawn and despawn is still there, in order; the oldest cosmetics went first.
        let kept: Vec<u64> = inbox
            .iter()
            .filter_map(|(_, g)| match g {
                Game::Spawn(s) => Some(s.guid),
                _ => None,
            })
            .collect();
        assert_eq!(kept, (0..50).collect::<Vec<_>>());
        assert_eq!(
            inbox
                .iter()
                .filter(|(_, g)| matches!(g, Game::Despawn { .. }))
                .count(),
            50
        );
        assert!(matches!(inbox.last(), Some((_, Game::Despawn { guid: 49, .. }))));
        let numbers = inbox
            .iter()
            .filter(|(_, g)| matches!(g, Game::Number { .. }))
            .count();
        assert!(numbers < 50);
        // Nothing sheddable left and still over: nothing essential is dropped to fit.
        let mut all_essential: Vec<(u8, Game)> = (0..30).map(|g| (0, spawn(g))).collect();
        trim_inbox(&mut all_essential, 10);
        assert_eq!(all_essential.len(), 30);
    }

    #[test]
    fn the_wire_knows_every_zone() {
        assert_eq!(usize::from(ZONE_COUNT), data::MAPS.len());
    }

    #[test]
    fn lobby_settings_become_rules() {
        let settings = Settings {
            duration_secs: 600,
            ambush_every_secs: 90,
            difficulty: Difficulty::HARD,
            shared_xp: false,
            revive: true,
            ..Settings::default()
        };
        let r = rules_for(&settings, 3);
        assert_eq!(r.duration, 600.0);
        assert_eq!(r.ambush_every, 90.0);
        assert_eq!(r.health, 1.5);
        assert_eq!(r.heroes, 3);
        assert!(!r.shared_xp && r.revive);
        assert_eq!(
            rules_for(&Settings::default(), 1).duration,
            data::FINAL_BOSS_SECS
        );
    }
}
