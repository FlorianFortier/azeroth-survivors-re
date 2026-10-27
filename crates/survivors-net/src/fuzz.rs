//! The codec and the hub against what a broken or hostile peer sends: random bytes, real messages
//! with bits flipped, cut short, padded or given forged lengths, and random message sequences from
//! many connections at once. Nothing here may panic, a frame that decodes must encode back to
//! itself, a bad message in a batch costs only itself, and the hub keeps its rules whatever order
//! the messages come in. Deterministic: a failure names its seed.

use crate::game::{decode_batch, encode_batch, Game};
use crate::hub::{ConnId, Hub, Limits};
use crate::protocol::{Down, LobbyView, Target, Up, PROTOCOL_VERSION};
use crate::settings::{Settings, MAX_PLAYERS};
use crate::wire::Wire;

/// xorshift64*: enough to fuzz with, and the same on every machine.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
    fn bytes(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.next() as u8).collect()
    }
}

/// One of the ways a frame goes wrong.
fn mutate(rng: &mut Rng, frame: &[u8]) -> Vec<u8> {
    let mut f = frame.to_vec();
    match rng.below(7) {
        // A bit flipped.
        0 if !f.is_empty() => {
            let i = rng.below(f.len());
            f[i] ^= 1 << rng.below(8);
        }
        // A byte replaced.
        1 if !f.is_empty() => {
            let i = rng.below(f.len());
            f[i] = rng.next() as u8;
        }
        // Cut short.
        2 => f.truncate(rng.below(f.len() + 1)),
        // Garbage after it.
        3 => {
            let n = 1 + rng.below(16);
            f.extend(rng.bytes(n));
        }
        // Bytes dropped from the middle.
        4 if f.len() > 2 => {
            let i = rng.below(f.len() - 1);
            let n = 1 + rng.below(f.len() - i);
            f.drain(i..i + n);
        }
        // A forged length: the most a u16 or u32 prefix can claim.
        5 if f.len() > 4 => {
            let i = rng.below(f.len() - 3);
            for b in &mut f[i..i + 4] {
                *b = 0xFF;
            }
        }
        // Several flips at once.
        _ => {
            for _ in 0..1 + rng.below(6) {
                if !f.is_empty() {
                    let i = rng.below(f.len());
                    f[i] = rng.next() as u8;
                }
            }
        }
    }
    f
}

/// A frame that decodes encodes back to the same message: a corrupted frame never decodes into
/// something that would read differently a second time. Compared as bytes, since a flipped bit
/// can make a NaN, which equals nothing.
fn stable<T: Wire + std::fmt::Debug>(frame: &[u8]) {
    if let Ok(m) = T::decode(frame) {
        let bytes = m.encode();
        let again = T::decode(&bytes).expect("a decoded message encodes and decodes");
        assert_eq!(again.encode(), bytes, "{m:?}");
    }
}

#[test]
fn random_bytes_never_panic_the_decoders() {
    let mut rng = Rng(0x5EED_0001);
    for _ in 0..20_000 {
        let n = rng.below(600);
        let f = rng.bytes(n);
        stable::<Up>(&f);
        stable::<Down>(&f);
        stable::<Game>(&f);
        let _ = decode_batch(&f);
    }
}

#[test]
fn corrupted_messages_never_panic_and_never_mislead() {
    let mut rng = Rng(0x5EED_0002);
    let ups: Vec<Vec<u8>> = crate::protocol::tests::up_samples()
        .iter()
        .map(|m| m.encode())
        .collect();
    let downs: Vec<Vec<u8>> = crate::protocol::tests::down_samples()
        .iter()
        .map(|m| m.encode())
        .collect();
    let games: Vec<Vec<u8>> = crate::game::tests::samples()
        .iter()
        .map(|m| m.encode())
        .collect();
    for _ in 0..300 {
        for f in &ups {
            stable::<Up>(&mutate(&mut rng, f));
        }
        for f in &downs {
            stable::<Down>(&mutate(&mut rng, f));
        }
        for f in &games {
            stable::<Game>(&mutate(&mut rng, f));
        }
    }
}

#[test]
fn a_bad_message_costs_a_batch_only_itself() {
    let all = crate::game::tests::samples();
    let clean = encode_batch(&all);
    assert_eq!(decode_batch(&clean), all);
    // One message turned into garbage of the same length: every other one still arrives.
    let mut rng = Rng(0x5EED_0003);
    for k in 0..all.len() {
        let mut msgs: Vec<Vec<u8>> = all.iter().map(|m| m.encode()).collect();
        let n = msgs[k].len();
        msgs[k] = vec![0xEE; n];
        let mut w = crate::wire::Writer::new();
        for m in &msgs {
            w.bytes(m);
        }
        let got = decode_batch(&w.finish());
        let want: Vec<Game> = all
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != k)
            .map(|(_, m)| m.clone())
            .collect();
        assert_eq!(got, want, "message {k} spoiled");
    }
    // A batch cut short keeps the whole messages before the cut.
    for _ in 0..200 {
        let cut = rng.below(clean.len());
        let got = decode_batch(&clean[..cut]);
        assert!(got.len() <= all.len());
        assert_eq!(got[..], all[..got.len()]);
    }
}

/// Every view the hub sends holds its rules.
fn check_view(to: ConnId, v: &LobbyView, seen: &mut std::collections::HashMap<ConnId, String>) {
    assert!(v.members.len() <= MAX_PLAYERS as usize, "{v:?}");
    assert!(v.members.len() <= v.settings.max_players as usize, "{v:?}");
    let mut slots: Vec<u8> = v.members.iter().map(|m| m.slot).collect();
    slots.sort_unstable();
    slots.dedup();
    assert_eq!(
        slots.len(),
        v.members.len(),
        "two members in one seat: {v:?}"
    );
    assert!(
        v.members.iter().any(|m| m.slot == v.you),
        "a view names a seat its reader is not in: {v:?}"
    );
    assert!(
        v.members.iter().any(|m| m.slot == 0),
        "a lobby with no host: {v:?}"
    );
    seen.insert(to, v.code.clone());
}

#[test]
fn the_hub_keeps_its_rules_whatever_comes_in_whatever_order() {
    let (mut runs, mut relays) = (0, 0);

    for seed in 1..=40u64 {
        let mut rng = Rng(0xC0FFEE ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut hub = Hub::new(seed, Limits::default());
        let conns: Vec<ConnId> = (1..=7).collect();
        let mut online: Vec<ConnId> = Vec::new();
        let mut codes: Vec<String> = Vec::new();
        let mut in_lobby: std::collections::HashMap<ConnId, String> = Default::default();
        for step in 0..1500 {
            let c = conns[rng.below(conns.len())];
            if !online.contains(&c) {
                // As a real client does, it says hello first (now and then on the wrong version).
                hub.connect(c);
                online.push(c);
                let _ = hub.handle(
                    c,
                    Up::Hello {
                        version: if rng.below(20) == 0 {
                            0
                        } else {
                            PROTOCOL_VERSION
                        },
                        name: format!("P{c}"),
                    },
                );
            }
            // Mostly the moves of a table at play, now and then a drop: joins, readies and
            // relays weigh most, so whole tables form, start and talk.
            const WEIGHTS: [usize; 16] = [1, 2, 6, 6, 1, 6, 3, 1, 1, 1, 8, 8, 1, 1, 1, 1];
            let pick = {
                let mut at = rng.below(WEIGHTS.iter().sum());
                WEIGHTS
                    .iter()
                    .position(|&w| {
                        let hit = at < w;
                        at = at.saturating_sub(w);
                        hit
                    })
                    .unwrap_or(15)
            };
            let msg = match pick {
                0 => Up::Hello {
                    version: if rng.below(10) == 0 {
                        0
                    } else {
                        PROTOCOL_VERSION
                    },
                    name: format!("P{c}"),
                },
                1 => Up::Create {
                    settings: Settings::default(),
                    loadout: Default::default(),
                },
                2 | 3 => Up::Join {
                    code: if rng.below(8) == 0 || codes.is_empty() {
                        "ZZZZZ".into()
                    } else {
                        codes[codes.len() - 1 - rng.below(codes.len().min(3))].clone()
                    },
                    loadout: Default::default(),
                },
                4 => Up::Leave,
                5 => Up::SetReady(rng.below(2) == 0),
                6 => Up::Start,
                7 => Up::EndRun,
                8 => Up::Kick(rng.below(5) as u8),
                9 => Up::UpdateSettings(Settings {
                    max_players: rng.below(6) as u8,
                    ..Settings::default()
                }),
                10 | 11 => Up::Relay {
                    to: match rng.below(3) {
                        0 => Target::Host,
                        1 => Target::Others,
                        _ => Target::Slot(rng.below(5) as u8),
                    },
                    data: {
                        let n = rng.below(64);
                        rng.bytes(n)
                    },
                },
                12 => Up::List,
                13 => Up::Ping(step),
                14 => Up::SetLoadout(Default::default()),
                _ => {
                    // The connection drops.
                    let out = hub.disconnect(c);
                    online.retain(|&o| o != c);
                    in_lobby.remove(&c);
                    for s in out {
                        assert_ne!(s.to, c, "a dropped connection is sent nothing");
                        if let Down::Lobby(v) = &s.msg {
                            check_view(s.to, v, &mut in_lobby);
                        }
                    }
                    continue;
                }
            };
            let relay_from_lobby = in_lobby.get(&c).cloned();
            let out = hub.handle(c, msg.clone());

            for s in &out {
                assert!(
                    online.contains(&s.to),
                    "seed {seed} step {step}: a frame for a connection that is gone"
                );
                match &s.msg {
                    Down::Lobby(v) => {
                        if !codes.contains(&v.code) {
                            codes.push(v.code.clone());
                        }
                        check_view(s.to, v, &mut in_lobby);
                    }
                    Down::Left(_) => {
                        in_lobby.remove(&s.to);
                    }
                    Down::Started { .. } => runs += 1,
                    Down::Relay { .. } => {
                        relays += 1;
                        // A relay reaches only the sender's own table.
                        assert!(matches!(msg, Up::Relay { .. }));
                        assert_ne!(s.to, c, "a relay never comes back to its sender");
                        assert_eq!(
                            in_lobby.get(&s.to),
                            relay_from_lobby.as_ref(),
                            "seed {seed} step {step}: a relay crossed to another lobby"
                        );
                    }
                    _ => {}
                }
            }
        }
        assert!(hub.connection_count() <= conns.len());
    }
    // The fuzz reached the deep states, not only the refusals.
    assert!(runs > 0 && relays > 0, "{runs} runs, {relays} relays");
    println!("{runs} runs started, {relays} relays delivered");
}
