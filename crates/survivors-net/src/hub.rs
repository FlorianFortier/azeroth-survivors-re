//! The lobby server's whole state and rules, with no sockets: a connection is a [`ConnId`], every
//! call returns the frames to send, and the server crate only moves bytes. Codes, seats, the
//! host's powers, readiness, the run and the relay are all decided here.

use std::collections::HashMap;

use crate::protocol::{
    normalize_code, Down, ErrorCode, LeftReason, Loadout, LobbyState, LobbySummary, LobbyView,
    Member, Slot, Target, Up, CODE_ALPHABET, CODE_LEN, MAX_FRAME, PROTOCOL_VERSION,
};
use crate::settings::{clean_name, Settings, MAX_PLAYERS, MAX_PLAYER_NAME};

/// One connection, as the server numbers it.
pub type ConnId = u64;

/// A frame to send.
#[derive(Debug, Clone, PartialEq)]
pub struct Send {
    pub to: ConnId,
    pub msg: Down,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    /// Lobbies the server holds at once.
    pub max_lobbies: usize,
    /// Lobbies listed per [`Up::List`] answer.
    pub max_listed: usize,
    /// Largest relayed payload.
    pub max_relay: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_lobbies: 1024,
            max_listed: 50,
            max_relay: MAX_FRAME - 64,
        }
    }
}

struct Conn {
    name: Option<String>,
    lobby: Option<String>,
}

struct Seat {
    /// None while the seat's player is away: their connection dropped during the run, and the
    /// seat waits for them to [`Up::Rejoin`].
    conn: Option<ConnId>,
    /// The player's name, kept for the seat while they are away.
    name: String,
    loadout: Loadout,
    ready: bool,
    /// This run's rejoin token for the seat, 0 outside a run.
    token: u64,
}

struct Lobby {
    code: String,
    settings: Settings,
    state: LobbyState,
    seats: [Option<Seat>; MAX_PLAYERS as usize],
}

impl Lobby {
    fn host(&self) -> Option<ConnId> {
        self.seats[0].as_ref().and_then(|s| s.conn)
    }
    fn count(&self) -> usize {
        self.seats.iter().flatten().count()
    }
    fn slot_of(&self, conn: ConnId) -> Option<Slot> {
        self.seats
            .iter()
            .position(|s| s.as_ref().is_some_and(|s| s.conn == Some(conn)))
            .map(|i| i as Slot)
    }
    fn conns(&self) -> impl Iterator<Item = (Slot, ConnId)> + '_ {
        self.seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().and_then(|s| s.conn).map(|c| (i as Slot, c)))
    }
}

pub struct Hub {
    conns: HashMap<ConnId, Conn>,
    lobbies: HashMap<String, Lobby>,
    rng: u64,
    limits: Limits,
}

impl Hub {
    /// `seed` drives the lobby codes and run seeds; the server passes the clock.
    pub fn new(seed: u64, limits: Limits) -> Self {
        Self {
            conns: HashMap::new(),
            lobbies: HashMap::new(),
            rng: seed | 1,
            limits,
        }
    }

    pub fn lobby_count(&self) -> usize {
        self.lobbies.len()
    }

    pub fn connection_count(&self) -> usize {
        self.conns.len()
    }

    fn next(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn new_code(&mut self) -> String {
        loop {
            let code: String = (0..CODE_LEN)
                .map(|_| CODE_ALPHABET[(self.next() % CODE_ALPHABET.len() as u64) as usize] as char)
                .collect();
            if !self.lobbies.contains_key(&code) {
                return code;
            }
        }
    }

    pub fn connect(&mut self, conn: ConnId) {
        self.conns.insert(
            conn,
            Conn {
                name: None,
                lobby: None,
            },
        );
    }

    /// The connection is gone: it leaves its lobby as if it had asked to, except a guest's seat
    /// in a run, which is kept away for its player to rejoin by its token.
    pub fn disconnect(&mut self, conn: ConnId) -> Vec<Send> {
        let mut out = Vec::new();
        let in_run = self
            .seat_of(conn)
            .filter(|(code, slot)| *slot != 0 && self.lobbies[code].state == LobbyState::InRun);
        match in_run {
            Some((code, slot)) => {
                if let Some(seat) = self
                    .lobbies
                    .get_mut(&code)
                    .and_then(|l| l.seats[slot as usize].as_mut())
                {
                    seat.conn = None;
                }
                self.conns.remove(&conn);
                self.broadcast_view(&code, &mut out);
            }
            None => {
                self.leave(conn, LeftReason::Left, false, &mut out);
                self.conns.remove(&conn);
            }
        }
        out
    }

    /// A frame that did not decode, or was too large.
    pub fn bad_frame(&mut self, conn: ConnId) -> Vec<Send> {
        vec![Send {
            to: conn,
            msg: Down::Error(ErrorCode::BadFrame),
        }]
    }

    pub fn handle(&mut self, conn: ConnId, msg: Up) -> Vec<Send> {
        let mut out = Vec::new();
        let err = |out: &mut Vec<Send>, code| {
            out.push(Send {
                to: conn,
                msg: Down::Error(code),
            })
        };
        let Some(c) = self.conns.get_mut(&conn) else {
            return out;
        };
        if c.name.is_none() {
            match msg {
                Up::Hello { version, name } => {
                    if version != PROTOCOL_VERSION {
                        err(&mut out, ErrorCode::Version);
                        return out;
                    }
                    let name = clean_name(&name, MAX_PLAYER_NAME);
                    c.name = Some(if name.is_empty() {
                        format!("Hero {}", conn % 1000)
                    } else {
                        name
                    });
                    out.push(Send {
                        to: conn,
                        msg: Down::Welcome {
                            version: PROTOCOL_VERSION,
                        },
                    });
                }
                _ => err(&mut out, ErrorCode::NoHello),
            }
            return out;
        }

        match msg {
            Up::Hello { .. } => err(&mut out, ErrorCode::BadState),
            Up::Ping(n) => out.push(Send {
                to: conn,
                msg: Down::Pong(n),
            }),
            Up::List => {
                let mut list: Vec<LobbySummary> = self
                    .lobbies
                    .values()
                    .filter(|l| l.settings.listed && l.state == LobbyState::Open)
                    .filter(|l| l.count() < l.settings.max_players as usize)
                    .map(|l| LobbySummary {
                        code: l.code.clone(),
                        name: l.settings.name.clone(),
                        host: l
                            .host()
                            .and_then(|h| self.conns.get(&h))
                            .and_then(|c| c.name.clone())
                            .unwrap_or_default(),
                        zone: l.settings.zone,
                        players: l.count() as u8,
                        max_players: l.settings.max_players,
                        difficulty: l.settings.difficulty.preset_name().to_string(),
                    })
                    .collect();
                list.sort_by(|a, b| a.name.cmp(&b.name).then(a.code.cmp(&b.code)));
                list.truncate(self.limits.max_listed);
                out.push(Send {
                    to: conn,
                    msg: Down::Lobbies(list),
                });
            }
            Up::Create { settings, loadout } => {
                self.leave(conn, LeftReason::Left, true, &mut out);
                if self.lobbies.len() >= self.limits.max_lobbies {
                    err(&mut out, ErrorCode::ServerFull);
                    return out;
                }
                let code = self.new_code();
                let name = self.conns[&conn].name.clone().unwrap_or_default();
                let mut seats: [Option<Seat>; MAX_PLAYERS as usize] = Default::default();
                seats[0] = Some(Seat {
                    conn: Some(conn),
                    name,
                    loadout,
                    ready: false,
                    token: 0,
                });
                self.lobbies.insert(
                    code.clone(),
                    Lobby {
                        code: code.clone(),
                        settings: settings.sanitized(),
                        state: LobbyState::Open,
                        seats,
                    },
                );
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.lobby = Some(code.clone());
                }
                self.broadcast_view(&code, &mut out);
            }
            Up::Join { code, loadout } => {
                let code = normalize_code(&code);
                if self.conns[&conn].lobby.as_deref() == Some(code.as_str()) {
                    self.broadcast_view(&code, &mut out);
                    return out;
                }
                let Some(l) = self.lobbies.get(&code) else {
                    err(&mut out, ErrorCode::NoSuchLobby);
                    return out;
                };
                if l.state != LobbyState::Open {
                    err(&mut out, ErrorCode::RunInProgress);
                    return out;
                }
                let max = l.settings.max_players as usize;
                let free = l.seats.iter().take(max).position(|s| s.is_none());
                let Some(slot) = free else {
                    err(&mut out, ErrorCode::LobbyFull);
                    return out;
                };
                self.leave(conn, LeftReason::Left, true, &mut out);
                let name = self.conns[&conn].name.clone().unwrap_or_default();
                if let Some(l) = self.lobbies.get_mut(&code) {
                    l.seats[slot] = Some(Seat {
                        conn: Some(conn),
                        name,
                        loadout,
                        ready: false,
                        token: 0,
                    });
                }
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.lobby = Some(code.clone());
                }
                self.broadcast_view(&code, &mut out);
            }
            Up::Leave => {
                if self.conns[&conn].lobby.is_none() {
                    err(&mut out, ErrorCode::NotInLobby);
                } else {
                    self.leave(conn, LeftReason::Left, true, &mut out);
                }
            }
            Up::UpdateSettings(settings) => {
                let Some((code, slot)) = self.seat_of(conn) else {
                    err(&mut out, ErrorCode::NotInLobby);
                    return out;
                };
                let l = self
                    .lobbies
                    .get_mut(&code)
                    .expect("seat_of names a live lobby");
                if slot != 0 {
                    err(&mut out, ErrorCode::NotHost);
                    return out;
                }
                if l.state != LobbyState::Open {
                    err(&mut out, ErrorCode::BadState);
                    return out;
                }
                let mut s = settings.sanitized();
                // Never fewer seats than are taken, nor a taken seat left past the end.
                let last_taken = l.seats.iter().rposition(|s| s.is_some()).unwrap_or(0) + 1;
                s.max_players = s.max_players.max(last_taken as u8);
                if s != l.settings {
                    l.settings = s;
                    // The rules changed under them: everyone confirms again.
                    for seat in l.seats.iter_mut().flatten() {
                        seat.ready = false;
                    }
                }
                self.broadcast_view(&code, &mut out);
            }
            Up::SetLoadout(loadout) => {
                let Some((code, slot)) = self.seat_of(conn) else {
                    err(&mut out, ErrorCode::NotInLobby);
                    return out;
                };
                let l = self
                    .lobbies
                    .get_mut(&code)
                    .expect("seat_of names a live lobby");
                if l.state != LobbyState::Open {
                    err(&mut out, ErrorCode::BadState);
                    return out;
                }
                if let Some(seat) = l.seats[slot as usize].as_mut() {
                    seat.loadout = loadout;
                }
                self.broadcast_view(&code, &mut out);
            }
            Up::SetReady(ready) => {
                let Some((code, slot)) = self.seat_of(conn) else {
                    err(&mut out, ErrorCode::NotInLobby);
                    return out;
                };
                let l = self
                    .lobbies
                    .get_mut(&code)
                    .expect("seat_of names a live lobby");
                if l.state != LobbyState::Open {
                    err(&mut out, ErrorCode::BadState);
                    return out;
                }
                if let Some(seat) = l.seats[slot as usize].as_mut() {
                    seat.ready = ready;
                }
                self.broadcast_view(&code, &mut out);
            }
            Up::Kick(target) => {
                let Some((code, slot)) = self.seat_of(conn) else {
                    err(&mut out, ErrorCode::NotInLobby);
                    return out;
                };
                if slot != 0 {
                    err(&mut out, ErrorCode::NotHost);
                    return out;
                }
                let victim = self.lobbies[&code]
                    .seats
                    .get(target as usize)
                    .and_then(|s| s.as_ref())
                    .map(|s| s.conn);
                match victim {
                    Some(Some(v)) if target != 0 => {
                        self.leave(v, LeftReason::Kicked, true, &mut out)
                    }
                    // A seat whose player is away: given up, for good.
                    Some(None) if target != 0 => {
                        if let Some(l) = self.lobbies.get_mut(&code) {
                            l.seats[target as usize] = None;
                        }
                        self.broadcast_view(&code, &mut out);
                    }
                    _ => err(&mut out, ErrorCode::BadState),
                }
            }
            Up::Start => {
                let Some((code, slot)) = self.seat_of(conn) else {
                    err(&mut out, ErrorCode::NotInLobby);
                    return out;
                };
                if slot != 0 {
                    err(&mut out, ErrorCode::NotHost);
                    return out;
                }
                let seed = self.next();
                let l = self
                    .lobbies
                    .get_mut(&code)
                    .expect("seat_of names a live lobby");
                if l.state != LobbyState::Open {
                    err(&mut out, ErrorCode::BadState);
                    return out;
                }
                if !l
                    .conns()
                    .all(|(s, _)| s == 0 || l.seats[s as usize].as_ref().is_some_and(|x| x.ready))
                {
                    err(&mut out, ErrorCode::NotReady);
                    return out;
                }
                l.state = LobbyState::InRun;
                let slots: Vec<usize> = (0..l.seats.len())
                    .filter(|&i| l.seats[i].is_some())
                    .collect();
                // Each seat's own rejoin token, never 0.
                let tokens: Vec<u64> = slots.iter().map(|_| self.next() | 1).collect();
                let l = self
                    .lobbies
                    .get_mut(&code)
                    .expect("seat_of names a live lobby");
                let mut to: Vec<(ConnId, u64)> = Vec::new();
                for (&i, &token) in slots.iter().zip(&tokens) {
                    if let Some(seat) = l.seats[i].as_mut() {
                        seat.token = token;
                        if let Some(c) = seat.conn {
                            to.push((c, token));
                        }
                    }
                }
                self.broadcast_view(&code, &mut out);
                for (c, token) in to {
                    out.push(Send {
                        to: c,
                        msg: Down::Started { seed, token },
                    });
                }
            }
            Up::EndRun => {
                let Some((code, slot)) = self.seat_of(conn) else {
                    err(&mut out, ErrorCode::NotInLobby);
                    return out;
                };
                if slot != 0 {
                    err(&mut out, ErrorCode::NotHost);
                    return out;
                }
                let l = self
                    .lobbies
                    .get_mut(&code)
                    .expect("seat_of names a live lobby");
                if l.state != LobbyState::InRun {
                    err(&mut out, ErrorCode::BadState);
                    return out;
                }
                l.state = LobbyState::Open;
                // A seat still away when the run ends is given up; the others' tokens expire.
                for seat in l.seats.iter_mut() {
                    if seat.as_ref().is_some_and(|s| s.conn.is_none()) {
                        *seat = None;
                    }
                }
                for seat in l.seats.iter_mut().flatten() {
                    seat.ready = false;
                    seat.token = 0;
                }
                let to: Vec<ConnId> = l.conns().map(|(_, c)| c).collect();
                for c in to {
                    out.push(Send {
                        to: c,
                        msg: Down::Ended,
                    });
                }
                self.broadcast_view(&code, &mut out);
            }
            Up::Rejoin { code, token } => {
                let code = normalize_code(&code);
                let seat = self.lobbies.get(&code).and_then(|l| {
                    (l.state == LobbyState::InRun && token != 0)
                        .then(|| {
                            l.seats.iter().position(|s| {
                                s.as_ref()
                                    .is_some_and(|s| s.conn.is_none() && s.token == token)
                            })
                        })
                        .flatten()
                });
                let Some(slot) = seat else {
                    err(&mut out, ErrorCode::CannotRejoin);
                    return out;
                };
                // Out of wherever this connection sat, into its seat again.
                self.leave(conn, LeftReason::Left, true, &mut out);
                let name = self.conns[&conn].name.clone();
                if let Some(seat) = self
                    .lobbies
                    .get_mut(&code)
                    .and_then(|l| l.seats[slot].as_mut())
                {
                    seat.conn = Some(conn);
                    if let Some(name) = name {
                        seat.name = name;
                    }
                }
                if let Some(c) = self.conns.get_mut(&conn) {
                    c.lobby = Some(code.clone());
                }
                out.push(Send {
                    to: conn,
                    msg: Down::Rejoined,
                });
                self.broadcast_view(&code, &mut out);
            }
            Up::Relay { to, data } => {
                let Some((code, from)) = self.seat_of(conn) else {
                    err(&mut out, ErrorCode::NotInLobby);
                    return out;
                };
                if data.len() > self.limits.max_relay {
                    err(&mut out, ErrorCode::BadFrame);
                    return out;
                }
                let l = &self.lobbies[&code];
                if l.state != LobbyState::InRun {
                    // Late frames from a run that just ended are dropped quietly.
                    return out;
                }
                let targets: Vec<ConnId> = match to {
                    Target::Host => l.host().into_iter().filter(|&h| h != conn).collect(),
                    Target::Slot(s) => l
                        .seats
                        .get(s as usize)
                        .and_then(|x| x.as_ref())
                        .and_then(|x| x.conn)
                        .into_iter()
                        .filter(|&c| c != conn)
                        .collect(),
                    Target::Others => l.conns().map(|(_, c)| c).filter(|&c| c != conn).collect(),
                };
                for t in targets {
                    out.push(Send {
                        to: t,
                        msg: Down::Relay {
                            from,
                            data: data.clone(),
                        },
                    });
                }
            }
        }
        out
    }

    fn seat_of(&self, conn: ConnId) -> Option<(String, Slot)> {
        let code = self.conns.get(&conn)?.lobby.clone()?;
        let slot = self.lobbies.get(&code)?.slot_of(conn)?;
        Some((code, slot))
    }

    /// Take `conn` out of its lobby. A host leaving closes the lobby for everyone. `tell` sends the
    /// leaver their own [`Down::Left`] (a dropped connection has no one to tell).
    fn leave(&mut self, conn: ConnId, reason: LeftReason, tell: bool, out: &mut Vec<Send>) {
        let Some((code, slot)) = self.seat_of(conn) else {
            if let Some(c) = self.conns.get_mut(&conn) {
                c.lobby = None;
            }
            return;
        };
        if let Some(c) = self.conns.get_mut(&conn) {
            c.lobby = None;
        }
        if tell {
            out.push(Send {
                to: conn,
                msg: Down::Left(reason),
            });
        }
        if slot == 0 {
            let Some(l) = self.lobbies.remove(&code) else {
                return;
            };
            for (_, c) in l.conns().filter(|&(_, c)| c != conn) {
                if let Some(cc) = self.conns.get_mut(&c) {
                    cc.lobby = None;
                }
                out.push(Send {
                    to: c,
                    msg: Down::Left(LeftReason::HostLeft),
                });
            }
        } else {
            if let Some(l) = self.lobbies.get_mut(&code) {
                l.seats[slot as usize] = None;
            }
            self.broadcast_view(&code, out);
        }
    }

    fn broadcast_view(&self, code: &str, out: &mut Vec<Send>) {
        let Some(l) = self.lobbies.get(code) else {
            return;
        };
        let members: Vec<Member> = l
            .seats
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                s.as_ref().map(|s| Member {
                    slot: i as Slot,
                    name: s.name.clone(),
                    loadout: s.loadout.clone(),
                    ready: s.ready,
                    away: s.conn.is_none(),
                })
            })
            .collect();
        for (you, conn) in l.conns() {
            out.push(Send {
                to: conn,
                msg: Down::Lobby(LobbyView {
                    code: l.code.clone(),
                    settings: l.settings.clone(),
                    state: l.state,
                    you,
                    members: members.clone(),
                }),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Difficulty;

    fn hub() -> Hub {
        Hub::new(12345, Limits::default())
    }

    fn hello(h: &mut Hub, conn: ConnId, name: &str) {
        h.connect(conn);
        let out = h.handle(
            conn,
            Up::Hello {
                version: PROTOCOL_VERSION,
                name: name.into(),
            },
        );
        assert_eq!(
            out,
            vec![Send {
                to: conn,
                msg: Down::Welcome {
                    version: PROTOCOL_VERSION
                }
            }]
        );
    }

    fn view_for(out: &[Send], conn: ConnId) -> LobbyView {
        out.iter()
            .rev()
            .find_map(|s| match &s.msg {
                Down::Lobby(v) if s.to == conn => Some(v.clone()),
                _ => None,
            })
            .expect("a lobby view for that connection")
    }

    fn errors(out: &[Send], conn: ConnId) -> Vec<ErrorCode> {
        out.iter()
            .filter(|s| s.to == conn)
            .filter_map(|s| match s.msg {
                Down::Error(e) => Some(e),
                _ => None,
            })
            .collect()
    }

    /// Host 1 creates; returns the code.
    fn create(h: &mut Hub, settings: Settings) -> String {
        let out = h.handle(
            1,
            Up::Create {
                settings,
                loadout: Loadout::default(),
            },
        );
        view_for(&out, 1).code
    }

    fn join(h: &mut Hub, conn: ConnId, code: &str) -> Vec<Send> {
        h.handle(
            conn,
            Up::Join {
                code: code.into(),
                loadout: Loadout::default(),
            },
        )
    }

    #[test]
    fn nothing_before_hello_and_versions_must_match() {
        let mut h = hub();
        h.connect(1);
        assert_eq!(errors(&h.handle(1, Up::List), 1), vec![ErrorCode::NoHello]);
        let out = h.handle(
            1,
            Up::Hello {
                version: PROTOCOL_VERSION + 1,
                name: "x".into(),
            },
        );
        assert_eq!(errors(&out, 1), vec![ErrorCode::Version]);
    }

    #[test]
    fn create_then_join_by_code_seats_in_order() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "Guest");
        let code = create(&mut h, Settings::default());
        assert_eq!(code.len(), CODE_LEN);
        assert!(code.bytes().all(|b| CODE_ALPHABET.contains(&b)));
        let out = join(&mut h, 2, &code.to_lowercase());
        let host_view = view_for(&out, 1);
        let guest_view = view_for(&out, 2);
        assert_eq!(host_view.you, 0);
        assert_eq!(guest_view.you, 1);
        assert!(host_view.is_host() && !guest_view.is_host());
        assert_eq!(guest_view.members.len(), 2);
        assert_eq!(guest_view.members[1].name, "Guest");
    }

    #[test]
    fn unknown_codes_full_lobbies_and_runs_refuse_joins() {
        let mut h = hub();
        for c in 1..=4 {
            hello(&mut h, c, "p");
        }
        assert_eq!(
            errors(&join(&mut h, 2, "ZZZZZ"), 2),
            vec![ErrorCode::NoSuchLobby]
        );
        let code = create(
            &mut h,
            Settings {
                max_players: 2,
                ..Settings::default()
            },
        );
        join(&mut h, 2, &code);
        assert_eq!(
            errors(&join(&mut h, 3, &code), 3),
            vec![ErrorCode::LobbyFull]
        );
        h.handle(2, Up::SetReady(true));
        h.handle(1, Up::Start);
        h.handle(2, Up::Leave);
        assert_eq!(
            errors(&join(&mut h, 3, &code), 3),
            vec![ErrorCode::RunInProgress]
        );
    }

    #[test]
    fn only_the_host_tunes_and_settings_are_clamped() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "Guest");
        hello(&mut h, 3, "Third");
        let code = create(&mut h, Settings::default());
        join(&mut h, 2, &code);
        join(&mut h, 3, &code);
        h.handle(2, Up::SetReady(true));
        let out = h.handle(2, Up::UpdateSettings(Settings::default()));
        assert_eq!(errors(&out, 2), vec![ErrorCode::NotHost]);

        let out = h.handle(
            1,
            Up::UpdateSettings(Settings {
                max_players: 1,
                zone: 250,
                difficulty: Difficulty::HARD,
                ..Settings::default()
            }),
        );
        let v = view_for(&out, 2);
        // Three seats are taken: the table cannot shrink under them.
        assert_eq!(v.settings.max_players, 3);
        assert_eq!(v.settings.zone, crate::settings::ZONE_COUNT - 1);
        assert_eq!(v.settings.difficulty, Difficulty::HARD);
        // A rules change clears everyone's ready.
        assert!(v.members.iter().all(|m| !m.ready));
    }

    /// The rejoin token a start gave `conn`.
    fn token_of(out: &[Send], conn: ConnId) -> u64 {
        out.iter()
            .find_map(|s| match s.msg {
                Down::Started { token, .. } if s.to == conn => Some(token),
                _ => None,
            })
            .expect("a start for that connection")
    }

    /// A host (1) and two guests (2, 3) in a run, and the guests' tokens.
    fn table_in_a_run(h: &mut Hub) -> (String, u64, u64) {
        hello(h, 1, "Host");
        hello(h, 2, "Guest");
        hello(h, 3, "Third");
        let code = create(h, Settings::default());
        join(h, 2, &code);
        join(h, 3, &code);
        h.handle(2, Up::SetReady(true));
        h.handle(3, Up::SetReady(true));
        let out = h.handle(1, Up::Start);
        let (t2, t3) = (token_of(&out, 2), token_of(&out, 3));
        assert!(t2 != 0 && t3 != 0 && t2 != t3, "each seat its own token");
        (code, t2, t3)
    }

    #[test]
    fn a_guest_who_drops_in_a_run_keeps_their_seat_and_rejoins_by_token() {
        let mut h = hub();
        let (code, t2, t3) = table_in_a_run(&mut h);
        // The guest's connection drops: its seat stays, away, under its name.
        let out = h.disconnect(2);
        let v = view_for(&out, 1);
        let m = v
            .members
            .iter()
            .find(|m| m.slot == 1)
            .expect("the seat stays");
        assert!(m.away && m.name == "Guest");
        assert!(!v.members.iter().find(|m| m.slot == 2).unwrap().away);
        // Nothing goes to an away seat.
        let out = h.handle(
            1,
            Up::Relay {
                to: Target::Slot(1),
                data: vec![1],
            },
        );
        assert!(out.is_empty());
        // A new connection: a wrong token, another seat's token, a token for no run are refused.
        hello(&mut h, 5, "Guest");
        for (c, t) in [
            (code.as_str(), t2 ^ 1),
            (code.as_str(), t3),
            ("ZZZZZ", t2),
            (code.as_str(), 0),
        ] {
            let out = h.handle(
                5,
                Up::Rejoin {
                    code: c.into(),
                    token: t,
                },
            );
            assert_eq!(errors(&out, 5), vec![ErrorCode::CannotRejoin], "{c} {t}");
        }
        // The right one takes the seat back: the same seat, no longer away.
        let out = h.handle(
            5,
            Up::Rejoin {
                code: code.to_lowercase(),
                token: t2,
            },
        );
        assert!(out.iter().any(|s| s.to == 5 && s.msg == Down::Rejoined));
        let mine = view_for(&out, 5);
        assert_eq!(mine.you, 1);
        assert!(mine.members.iter().all(|m| !m.away));
        assert!(!view_for(&out, 1).members[1].away);
        // And its relays flow from that seat again.
        let out = h.handle(
            5,
            Up::Relay {
                to: Target::Host,
                data: vec![7],
            },
        );
        assert_eq!(
            out,
            vec![Send {
                to: 1,
                msg: Down::Relay {
                    from: 1,
                    data: vec![7]
                }
            }]
        );
        // A token is good once: the seat is taken now.
        hello(&mut h, 6, "Impostor");
        let out = h.handle(6, Up::Rejoin { code, token: t2 });
        assert_eq!(errors(&out, 6), vec![ErrorCode::CannotRejoin]);
    }

    #[test]
    fn an_away_seat_is_given_up_when_the_run_ends_or_the_host_kicks_it() {
        let mut h = hub();
        let (code, t2, t3) = table_in_a_run(&mut h);
        h.disconnect(2);
        h.disconnect(3);
        // The host gives up on one.
        let out = h.handle(1, Up::Kick(2));
        assert_eq!(view_for(&out, 1).members.len(), 2);
        hello(&mut h, 5, "Third");
        let out = h.handle(
            5,
            Up::Rejoin {
                code: code.clone(),
                token: t3,
            },
        );
        assert_eq!(errors(&out, 5), vec![ErrorCode::CannotRejoin]);
        // The run ends: the other away seat goes too, and its token with it.
        let out = h.handle(1, Up::EndRun);
        let v = view_for(&out, 1);
        assert_eq!(v.members.len(), 1, "{v:?}");
        let out = h.handle(5, Up::Rejoin { code, token: t2 });
        assert_eq!(errors(&out, 5), vec![ErrorCode::CannotRejoin]);
    }

    #[test]
    fn leaving_on_purpose_frees_the_seat_and_a_host_dropping_closes_the_run() {
        let mut h = hub();
        let (code, t2, _) = table_in_a_run(&mut h);
        // A guest who leaves is gone, not away.
        let out = h.handle(3, Up::Leave);
        assert_eq!(view_for(&out, 1).members.len(), 2);
        // The host's connection drops: the run is over for everyone, and no seat waits.
        h.disconnect(2);
        let out = h.disconnect(1);
        assert!(out.is_empty() || out.iter().all(|s| s.to != 1));
        hello(&mut h, 5, "Guest");
        let out = h.handle(5, Up::Rejoin { code, token: t2 });
        assert_eq!(errors(&out, 5), vec![ErrorCode::CannotRejoin]);
        assert_eq!(h.lobby_count(), 0);
    }

    #[test]
    fn start_needs_everyone_ready_then_relays_flow() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "A");
        hello(&mut h, 3, "B");
        let code = create(&mut h, Settings::default());
        join(&mut h, 2, &code);
        join(&mut h, 3, &code);

        // Relays before the run are dropped.
        let out = h.handle(
            1,
            Up::Relay {
                to: Target::Others,
                data: vec![1],
            },
        );
        assert!(out.is_empty());

        h.handle(2, Up::SetReady(true));
        assert_eq!(
            errors(&h.handle(1, Up::Start), 1),
            vec![ErrorCode::NotReady]
        );
        assert_eq!(errors(&h.handle(2, Up::Start), 2), vec![ErrorCode::NotHost]);
        h.handle(3, Up::SetReady(true));
        let out = h.handle(1, Up::Start);
        let seeds: Vec<u64> = out
            .iter()
            .filter_map(|s| match s.msg {
                Down::Started { seed, .. } => Some(seed),
                _ => None,
            })
            .collect();
        assert_eq!(seeds.len(), 3);
        assert!(seeds.iter().all(|&s| s == seeds[0]));
        assert_eq!(view_for(&out, 3).state, LobbyState::InRun);

        // Host → everyone else.
        let out = h.handle(
            1,
            Up::Relay {
                to: Target::Others,
                data: vec![7],
            },
        );
        let mut to: Vec<ConnId> = out.iter().map(|s| s.to).collect();
        to.sort();
        assert_eq!(to, vec![2, 3]);
        assert!(out.iter().all(|s| s.msg
            == Down::Relay {
                from: 0,
                data: vec![7]
            }));

        // A guest → the host only, stamped with the guest's seat.
        let out = h.handle(
            3,
            Up::Relay {
                to: Target::Host,
                data: vec![9],
            },
        );
        assert_eq!(
            out,
            vec![Send {
                to: 1,
                msg: Down::Relay {
                    from: 2,
                    data: vec![9]
                }
            }]
        );

        // The host → one seat.
        let out = h.handle(
            1,
            Up::Relay {
                to: Target::Slot(1),
                data: vec![3],
            },
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].to, 2);

        // An oversized payload is refused.
        let out = h.handle(
            1,
            Up::Relay {
                to: Target::Others,
                data: vec![0; MAX_FRAME],
            },
        );
        assert_eq!(errors(&out, 1), vec![ErrorCode::BadFrame]);

        // End the run: back to an open lobby, nobody ready.
        let out = h.handle(1, Up::EndRun);
        assert!(out.iter().any(|s| s.to == 2 && s.msg == Down::Ended));
        let v = view_for(&out, 2);
        assert_eq!(v.state, LobbyState::Open);
        assert!(v.members.iter().all(|m| !m.ready));
    }

    #[test]
    fn the_host_leaving_closes_the_lobby() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "A");
        let code = create(&mut h, Settings::default());
        join(&mut h, 2, &code);
        let out = h.disconnect(1);
        assert_eq!(
            out,
            vec![Send {
                to: 2,
                msg: Down::Left(LeftReason::HostLeft)
            }]
        );
        assert_eq!(h.lobby_count(), 0);
        assert_eq!(
            errors(&h.handle(2, Up::SetReady(true)), 2),
            vec![ErrorCode::NotInLobby]
        );
    }

    #[test]
    fn a_guest_leaving_frees_the_seat_for_the_next() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "A");
        hello(&mut h, 3, "B");
        let code = create(&mut h, Settings::default());
        join(&mut h, 2, &code);
        let out = h.handle(2, Up::Leave);
        assert!(out.contains(&Send {
            to: 2,
            msg: Down::Left(LeftReason::Left)
        }));
        assert_eq!(view_for(&out, 1).members.len(), 1);
        let out = join(&mut h, 3, &code);
        assert_eq!(view_for(&out, 3).you, 1);
    }

    #[test]
    fn the_host_kicks_but_not_itself() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "A");
        let code = create(&mut h, Settings::default());
        join(&mut h, 2, &code);
        assert_eq!(
            errors(&h.handle(2, Up::Kick(0)), 2),
            vec![ErrorCode::NotHost]
        );
        assert_eq!(
            errors(&h.handle(1, Up::Kick(0)), 1),
            vec![ErrorCode::BadState]
        );
        let out = h.handle(1, Up::Kick(1));
        assert!(out.contains(&Send {
            to: 2,
            msg: Down::Left(LeftReason::Kicked)
        }));
        assert_eq!(view_for(&out, 1).members.len(), 1);
    }

    #[test]
    fn creating_or_joining_again_leaves_the_old_lobby() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "Other host");
        let a = create(&mut h, Settings::default());
        let out = h.handle(
            2,
            Up::Create {
                settings: Settings::default(),
                loadout: Loadout::default(),
            },
        );
        let b = view_for(&out, 2).code;
        assert_ne!(a, b);
        // Host 1 joins b: its own lobby a closes.
        join(&mut h, 1, &b);
        assert_eq!(h.lobby_count(), 1);
    }

    #[test]
    fn only_listed_open_lobbies_with_room_are_listed() {
        let mut h = hub();
        hello(&mut h, 1, "Host");
        hello(&mut h, 2, "Listed host");
        hello(&mut h, 3, "Asker");
        create(&mut h, Settings::default());
        h.handle(
            2,
            Up::Create {
                settings: Settings {
                    name: "Public run".into(),
                    listed: true,
                    ..Settings::default()
                },
                loadout: Loadout::default(),
            },
        );
        let out = h.handle(3, Up::List);
        let Down::Lobbies(list) = &out[0].msg else {
            panic!("expected a list")
        };
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Public run");
        assert_eq!(list[0].host, "Listed host");
        assert_eq!(list[0].players, 1);
        assert_eq!(list[0].difficulty, "Normal");
    }

    #[test]
    fn the_server_caps_its_lobbies() {
        let mut h = Hub::new(
            1,
            Limits {
                max_lobbies: 1,
                ..Limits::default()
            },
        );
        hello(&mut h, 1, "a");
        hello(&mut h, 2, "b");
        create(&mut h, Settings::default());
        let out = h.handle(
            2,
            Up::Create {
                settings: Settings::default(),
                loadout: Loadout::default(),
            },
        );
        assert_eq!(errors(&out, 2), vec![ErrorCode::ServerFull]);
    }

    #[test]
    fn codes_are_unique_across_many_lobbies() {
        let mut h = hub();
        let mut seen = std::collections::HashSet::new();
        for c in 1..=500 {
            hello(&mut h, c, "h");
            let out = h.handle(
                c,
                Up::Create {
                    settings: Settings::default(),
                    loadout: Loadout::default(),
                },
            );
            assert!(seen.insert(view_for(&out, c).code));
        }
        assert_eq!(h.lobby_count(), 500);
    }
}
