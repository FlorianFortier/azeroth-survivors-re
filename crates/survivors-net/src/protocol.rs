//! The lobby protocol: what a game sends the lobby server ([`Up`]) and what the server answers
//! ([`Down`]). One message per WebSocket binary frame. In a run, the game's own messages
//! ([`crate::game::Game`]) travel inside [`Up::Relay`] / [`Down::Relay`], opaque to the server.

use crate::settings::Settings;
use crate::wire::{Reader, Wire, WireError, Writer};

/// Bumped on any change to this protocol or to [`crate::game`]: a client and server that disagree
/// refuse each other at [`Up::Hello`].
pub const PROTOCOL_VERSION: u16 = 17;

/// Where the lobby lives on a server, under its origin.
pub const LOBBY_PATH: &str = "/lobby";

/// The largest frame either side accepts.
pub const MAX_FRAME: usize = 256 * 1024;

/// A seat at a lobby's table: 0 is always the host.
pub type Slot = u8;

/// Most bonuses and pieces of gear a loadout carries; past it, the rest are dropped.
pub const MAX_LOADOUT_BONUSES: usize = 128;
pub const MAX_LOADOUT_GEAR: usize = 16;
pub const MAX_LOADOUT_PERKS: usize = 32;
/// How many appearances a loadout carries, one per slot and some slack.
pub const MAX_LOADOUT_LOOKS: usize = 4;

/// The hero a player brings: an index into the game's classes, the look picked for it, and what
/// its profile brings into a run (the gear worn, the collection, the talents) for the host, who
/// plays every hero, to give it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Loadout {
    pub class: u8,
    /// `CreatureDisplayInfo` id of the look.
    pub display: u32,
    pub race: u8,
    pub sex: u8,
    /// The five character-creation dials: skin, face, hair style, hair colour, facial hair.
    pub appearance: [u8; 5],
    /// The profile's bonuses beyond the race's own: `(stat index, 0 flat or 1 percent, value in
    /// ten-thousandths)`.
    pub bonuses: Vec<(u8, u8, i32)>,
    /// The gear worn, by item name, for the host to dress the hero in.
    pub gear: Vec<String>,
    /// The perks bought, `(key, rank)`: the host holds each rank to its perk's most.
    pub perks: Vec<(String, u8)>,
    /// The tabard worn, by key; empty for none.
    pub tabard: String,
    /// The appearances worn on the head and back, by key.
    pub looks: Vec<String>,
    /// The companion at the hero's side, by key; empty for none.
    pub pet: String,
    /// What the companion runs for first.
    pub instinct: u8,
}

/// Who a relayed game message goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The lobby's host.
    Host,
    /// One seat.
    Slot(Slot),
    /// Everyone in the lobby but the sender.
    Others,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Up {
    /// The first message on a connection.
    Hello {
        version: u16,
        name: String,
    },
    Create {
        settings: Settings,
        loadout: Loadout,
    },
    Join {
        code: String,
        loadout: Loadout,
    },
    /// Ask for the server's listed, open lobbies.
    List,
    Leave,
    /// Host only, before the run.
    UpdateSettings(Settings),
    SetLoadout(Loadout),
    SetReady(bool),
    /// Host only: remove a seat.
    Kick(Slot),
    /// Host only: every other seat must be ready.
    Start,
    /// Host only: the run is over, back to the lobby.
    EndRun,
    /// A game message for others in the lobby, during a run.
    Relay {
        to: Target,
        data: Vec<u8>,
    },
    Ping(u32),
    /// Back to a run this connection's seat dropped out of, by the token [`Down::Started`] gave it.
    Rejoin {
        code: String,
        token: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LobbyState {
    Open,
    InRun,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub slot: Slot,
    pub name: String,
    pub loadout: Loadout,
    pub ready: bool,
    /// The seat's connection dropped during the run: it is kept for its player to rejoin.
    pub away: bool,
}

/// A lobby as one member sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct LobbyView {
    pub code: String,
    pub settings: Settings,
    pub state: LobbyState,
    /// The recipient's own seat.
    pub you: Slot,
    pub members: Vec<Member>,
}

impl LobbyView {
    pub fn is_host(&self) -> bool {
        self.you == 0
    }
    pub fn me(&self) -> Option<&Member> {
        self.members.iter().find(|m| m.slot == self.you)
    }
    /// Everyone but the host has readied.
    pub fn all_ready(&self) -> bool {
        self.members.iter().all(|m| m.slot == 0 || m.ready)
    }
}

/// A listed lobby, in the server's browser.
#[derive(Debug, Clone, PartialEq)]
pub struct LobbySummary {
    pub code: String,
    pub name: String,
    pub host: String,
    pub zone: u8,
    pub players: u8,
    pub max_players: u8,
    pub difficulty: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// The client and server speak different protocol versions.
    Version,
    /// Something other than [`Up::Hello`] came first.
    NoHello,
    NoSuchLobby,
    LobbyFull,
    /// The lobby's run has started; it opens again when it ends.
    RunInProgress,
    /// Only the host may do that.
    NotHost,
    NotInLobby,
    /// Not every seat is ready.
    NotReady,
    /// The server holds as many lobbies as it will.
    ServerFull,
    /// That is not something to do in the lobby's current state.
    BadState,
    /// A frame the server could not read, or too large.
    BadFrame,
    /// No seat of that run waits for that token: the run ended, or the seat was given up.
    CannotRejoin,
}

impl ErrorCode {
    pub fn message(self) -> &'static str {
        match self {
            ErrorCode::Version => "The server runs another version of the game.",
            ErrorCode::NoHello => "The connection did not introduce itself.",
            ErrorCode::NoSuchLobby => "No lobby has that code.",
            ErrorCode::LobbyFull => "That lobby is full.",
            ErrorCode::RunInProgress => "That lobby's run has already started.",
            ErrorCode::NotHost => "Only the host can do that.",
            ErrorCode::NotInLobby => "You are not in a lobby.",
            ErrorCode::NotReady => "Not every player is ready.",
            ErrorCode::ServerFull => "The server is full; try again later.",
            ErrorCode::BadState => "That cannot be done right now.",
            ErrorCode::BadFrame => "The server could not read a message.",
            ErrorCode::CannotRejoin => {
                "The run could not be rejoined: it ended, or the seat was given up."
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftReason {
    /// You asked to.
    Left,
    Kicked,
    /// The host left: the lobby is gone.
    HostLeft,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Down {
    Welcome {
        version: u16,
    },
    /// The lobby you are in, after any change to it.
    Lobby(LobbyView),
    Lobbies(Vec<LobbySummary>),
    /// The run begins; every seat plays it with this seed. The token is this seat's own: if its
    /// connection drops, [`Up::Rejoin`] with it takes the seat back.
    Started {
        seed: u64,
        token: u64,
    },
    /// The run ended; the lobby is open again.
    Ended,
    Relay {
        from: Slot,
        data: Vec<u8>,
    },
    Left(LeftReason),
    Error(ErrorCode),
    Pong(u32),
    /// This connection has its seat back in the run; the host sends it the world again.
    Rejoined,
}

/// Normalize a typed code: uppercase, separators and spaces dropped, look-alikes folded.
pub fn normalize_code(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| match c.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            c => c,
        })
        .collect()
}

/// The alphabet codes are drawn from: no 0/O, no 1/I/L, so a code read aloud or off a stream is
/// typed right ([`normalize_code`] folds those, and none of them is ever issued).
pub const CODE_ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
pub const CODE_LEN: usize = 5;

// --- codec ---------------------------------------------------------------------------------

impl Wire for Loadout {
    fn put(&self, w: &mut Writer) {
        w.u8(self.class)
            .u32(self.display)
            .u8(self.race)
            .u8(self.sex);
        for d in self.appearance {
            w.u8(d);
        }
        let bonuses = &self.bonuses[..self.bonuses.len().min(MAX_LOADOUT_BONUSES)];
        w.len(bonuses.len());
        for &(stat, kind, value) in bonuses {
            w.u8(stat).u8(kind).u32(value as u32);
        }
        let gear = &self.gear[..self.gear.len().min(MAX_LOADOUT_GEAR)];
        w.len(gear.len());
        for name in gear {
            w.str(name);
        }
        let perks = &self.perks[..self.perks.len().min(MAX_LOADOUT_PERKS)];
        w.len(perks.len());
        for (key, rank) in perks {
            w.str(key).u8(*rank);
        }
        w.str(&self.tabard);
        let looks = &self.looks[..self.looks.len().min(MAX_LOADOUT_LOOKS)];
        w.len(looks.len());
        for key in looks {
            w.str(key);
        }
        w.str(&self.pet).u8(self.instinct);
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            class: r.u8()?,
            display: r.u32()?,
            race: r.u8()?,
            sex: r.u8()?,
            appearance: [r.u8()?, r.u8()?, r.u8()?, r.u8()?, r.u8()?],
            bonuses: {
                let n = r.len(6)?.min(MAX_LOADOUT_BONUSES);
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push((r.u8()?, r.u8()?, r.u32()? as i32));
                }
                out
            },
            gear: {
                let n = r.len(2)?.min(MAX_LOADOUT_GEAR);
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(r.str()?);
                }
                out
            },
            perks: {
                let n = r.len(3)?.min(MAX_LOADOUT_PERKS);
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push((r.str()?, r.u8()?));
                }
                out
            },
            tabard: r.str()?,
            looks: {
                let n = r.len(2)?.min(MAX_LOADOUT_LOOKS);
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    out.push(r.str()?);
                }
                out
            },
            pet: r.str()?,
            instinct: r.u8()?,
        })
    }
}

impl Wire for Target {
    fn put(&self, w: &mut Writer) {
        match self {
            Target::Host => w.u8(0),
            Target::Slot(s) => w.u8(1).u8(*s),
            Target::Others => w.u8(2),
        };
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        match r.u8()? {
            0 => Ok(Target::Host),
            1 => Ok(Target::Slot(r.u8()?)),
            2 => Ok(Target::Others),
            t => Err(WireError::BadTag(t)),
        }
    }
}

impl Wire for Up {
    fn put(&self, w: &mut Writer) {
        match self {
            Up::Hello { version, name } => {
                w.u8(0).u16(*version).str(name);
            }
            Up::Create { settings, loadout } => {
                w.u8(1);
                settings.put(w);
                loadout.put(w);
            }
            Up::Join { code, loadout } => {
                w.u8(2).str(code);
                loadout.put(w);
            }
            Up::List => {
                w.u8(3);
            }
            Up::Leave => {
                w.u8(4);
            }
            Up::UpdateSettings(s) => {
                w.u8(5);
                s.put(w);
            }
            Up::SetLoadout(l) => {
                w.u8(6);
                l.put(w);
            }
            Up::SetReady(b) => {
                w.u8(7).bool(*b);
            }
            Up::Kick(s) => {
                w.u8(8).u8(*s);
            }
            Up::Start => {
                w.u8(9);
            }
            Up::EndRun => {
                w.u8(10);
            }
            Up::Relay { to, data } => {
                w.u8(11);
                to.put(w);
                w.bytes(data);
            }
            Up::Ping(n) => {
                w.u8(12).u32(*n);
            }
            Up::Rejoin { code, token } => {
                w.u8(13).str(code).u64(*token);
            }
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => Up::Hello {
                version: r.u16()?,
                name: r.str()?,
            },
            1 => Up::Create {
                settings: Settings::get(r)?,
                loadout: Loadout::get(r)?,
            },
            2 => Up::Join {
                code: r.str()?,
                loadout: Loadout::get(r)?,
            },
            3 => Up::List,
            4 => Up::Leave,
            5 => Up::UpdateSettings(Settings::get(r)?),
            6 => Up::SetLoadout(Loadout::get(r)?),
            7 => Up::SetReady(r.bool()?),
            8 => Up::Kick(r.u8()?),
            9 => Up::Start,
            10 => Up::EndRun,
            11 => Up::Relay {
                to: Target::get(r)?,
                data: r.bytes()?,
            },
            12 => Up::Ping(r.u32()?),
            13 => Up::Rejoin {
                code: r.str()?,
                token: r.u64()?,
            },
            t => return Err(WireError::BadTag(t)),
        })
    }
}

impl Wire for Member {
    fn put(&self, w: &mut Writer) {
        w.u8(self.slot).str(&self.name);
        self.loadout.put(w);
        w.bool(self.ready).bool(self.away);
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            slot: r.u8()?,
            name: r.str()?,
            loadout: Loadout::get(r)?,
            ready: r.bool()?,
            away: r.bool()?,
        })
    }
}

impl Wire for LobbyView {
    fn put(&self, w: &mut Writer) {
        w.str(&self.code);
        self.settings.put(w);
        w.u8(match self.state {
            LobbyState::Open => 0,
            LobbyState::InRun => 1,
        })
        .u8(self.you)
        .len(self.members.len());
        for m in &self.members {
            m.put(w);
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        let code = r.str()?;
        let settings = Settings::get(r)?;
        let state = match r.u8()? {
            0 => LobbyState::Open,
            1 => LobbyState::InRun,
            t => return Err(WireError::BadTag(t)),
        };
        let you = r.u8()?;
        let n = r.len(12)?;
        let mut members = Vec::with_capacity(n);
        for _ in 0..n {
            members.push(Member::get(r)?);
        }
        Ok(Self {
            code,
            settings,
            state,
            you,
            members,
        })
    }
}

impl Wire for LobbySummary {
    fn put(&self, w: &mut Writer) {
        w.str(&self.code)
            .str(&self.name)
            .str(&self.host)
            .u8(self.zone)
            .u8(self.players)
            .u8(self.max_players)
            .str(&self.difficulty);
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            code: r.str()?,
            name: r.str()?,
            host: r.str()?,
            zone: r.u8()?,
            players: r.u8()?,
            max_players: r.u8()?,
            difficulty: r.str()?,
        })
    }
}

impl ErrorCode {
    const ALL: [ErrorCode; 12] = [
        ErrorCode::Version,
        ErrorCode::NoHello,
        ErrorCode::NoSuchLobby,
        ErrorCode::LobbyFull,
        ErrorCode::RunInProgress,
        ErrorCode::NotHost,
        ErrorCode::NotInLobby,
        ErrorCode::NotReady,
        ErrorCode::ServerFull,
        ErrorCode::BadState,
        ErrorCode::BadFrame,
        ErrorCode::CannotRejoin,
    ];
}

impl Wire for Down {
    fn put(&self, w: &mut Writer) {
        match self {
            Down::Welcome { version } => {
                w.u8(0).u16(*version);
            }
            Down::Lobby(v) => {
                w.u8(1);
                v.put(w);
            }
            Down::Lobbies(list) => {
                w.u8(2).len(list.len());
                for l in list {
                    l.put(w);
                }
            }
            Down::Started { seed, token } => {
                w.u8(3).u64(*seed).u64(*token);
            }
            Down::Ended => {
                w.u8(4);
            }
            Down::Relay { from, data } => {
                w.u8(5).u8(*from).bytes(data);
            }
            Down::Left(reason) => {
                w.u8(6).u8(match reason {
                    LeftReason::Left => 0,
                    LeftReason::Kicked => 1,
                    LeftReason::HostLeft => 2,
                });
            }
            Down::Error(code) => {
                let i = ErrorCode::ALL.iter().position(|c| c == code).unwrap_or(0);
                w.u8(7).u8(i as u8);
            }
            Down::Pong(n) => {
                w.u8(8).u32(*n);
            }
            Down::Rejoined => {
                w.u8(9);
            }
        }
    }
    fn get(r: &mut Reader) -> Result<Self, WireError> {
        Ok(match r.u8()? {
            0 => Down::Welcome { version: r.u16()? },
            1 => Down::Lobby(LobbyView::get(r)?),
            2 => {
                let n = r.len(12)?;
                let mut list = Vec::with_capacity(n);
                for _ in 0..n {
                    list.push(LobbySummary::get(r)?);
                }
                Down::Lobbies(list)
            }
            3 => Down::Started {
                seed: r.u64()?,
                token: r.u64()?,
            },
            4 => Down::Ended,
            5 => Down::Relay {
                from: r.u8()?,
                data: r.bytes()?,
            },
            6 => Down::Left(match r.u8()? {
                0 => LeftReason::Left,
                1 => LeftReason::Kicked,
                2 => LeftReason::HostLeft,
                t => return Err(WireError::BadTag(t)),
            }),
            7 => {
                let i = r.u8()?;
                Down::Error(*ErrorCode::ALL.get(i as usize).ok_or(WireError::BadTag(i))?)
            }
            8 => Down::Pong(r.u32()?),
            9 => Down::Rejoined,
            t => return Err(WireError::BadTag(t)),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn loadout() -> Loadout {
        Loadout {
            class: 3,
            display: 1234,
            race: 2,
            sex: 1,
            appearance: [1, 2, 3, 4, 5],
            bonuses: vec![(1, 1, 400), (9, 0, -50)],
            gear: vec!["Hogger's Helm".into()],
            perks: vec![("second_thoughts".into(), 2)],
            tabard: "frostwolf".into(),
            looks: vec!["helm_of_wrath".into(), "bare_back".into()],
            pet: "prairie_dog".into(),
            instinct: 1,
        }
    }

    /// One of every lobby message each way, for the round trip and the fuzzer.
    pub(crate) fn up_samples() -> Vec<Up> {
        vec![
            Up::Hello {
                version: PROTOCOL_VERSION,
                name: "Rexxar".into(),
            },
            Up::Create {
                settings: Settings::default(),
                loadout: loadout(),
            },
            Up::Join {
                code: "AB2CD".into(),
                loadout: loadout(),
            },
            Up::List,
            Up::Leave,
            Up::UpdateSettings(Settings::default()),
            Up::SetLoadout(loadout()),
            Up::SetReady(true),
            Up::Kick(2),
            Up::Start,
            Up::EndRun,
            Up::Relay {
                to: Target::Slot(3),
                data: vec![9, 8, 7],
            },
            Up::Relay {
                to: Target::Others,
                data: vec![],
            },
            Up::Ping(42),
            Up::Rejoin {
                code: "XK7QP".into(),
                token: 0xDEAD_BEEF_0042,
            },
        ]
    }

    #[test]
    fn every_up_round_trips() {
        for m in up_samples() {
            assert_eq!(Up::decode(&m.encode()).unwrap(), m);
        }
    }

    pub(crate) fn down_samples() -> Vec<Down> {
        let view = LobbyView {
            code: "XK7QP".into(),
            settings: Settings::default(),
            state: LobbyState::InRun,
            you: 1,
            members: vec![
                Member {
                    slot: 0,
                    name: "Host".into(),
                    loadout: loadout(),
                    ready: false,
                    away: false,
                },
                Member {
                    slot: 1,
                    name: "Guest".into(),
                    loadout: loadout(),
                    ready: true,
                    away: true,
                },
            ],
        };
        let mut all = vec![
            Down::Welcome { version: 1 },
            Down::Lobby(view),
            Down::Lobbies(vec![LobbySummary {
                code: "XK7QP".into(),
                name: "Raid".into(),
                host: "Host".into(),
                zone: 4,
                players: 2,
                max_players: 4,
                difficulty: "Hard".into(),
            }]),
            Down::Started {
                seed: 77,
                token: u64::MAX - 3,
            },
            Down::Rejoined,
            Down::Ended,
            Down::Relay {
                from: 0,
                data: vec![1, 2],
            },
            Down::Left(LeftReason::HostLeft),
            Down::Pong(5),
        ];
        all.extend(ErrorCode::ALL.iter().map(|&c| Down::Error(c)));
        all
    }

    #[test]
    fn every_down_round_trips() {
        for m in down_samples() {
            assert_eq!(Down::decode(&m.encode()).unwrap(), m);
        }
    }

    #[test]
    fn codes_are_normalized_and_unambiguous() {
        assert_eq!(normalize_code(" xk7-qp "), "XK7QP");
        assert_eq!(normalize_code("o1l"), "011");
        for c in CODE_ALPHABET {
            assert!(!b"01OIL".contains(c));
            assert_eq!(
                normalize_code(&(*c as char).to_string()),
                (*c as char).to_string()
            );
        }
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(Up::decode(&[200]).is_err());
        assert!(Down::decode(&[]).is_err());
        assert!(Up::decode(&[3, 0]).is_err());
    }
}
