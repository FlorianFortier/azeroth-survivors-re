//! The lobby server end to end over real sockets: two clients create and join a lobby by code,
//! start a run and exchange a relayed game batch; then the host leaves and the guest is told.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use survivors_net::game::{decode_batch, encode_batch, Game};
use survivors_net::protocol::{Down, LeftReason, Loadout, Target, Up, PROTOCOL_VERSION};
use survivors_net::settings::Settings;
use survivors_net::Wire;
use survivors_server::{router, Config, LobbyServer};
use tokio_tungstenite::tungstenite::Message;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn serve() -> (String, LobbyServer) {
    let server = LobbyServer::new(Config::default());
    let app = router(server.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("ws://{addr}/lobby"), server)
}

async fn connect(url: &str, name: &str) -> Ws {
    let (mut ws, _) = tokio_tungstenite::connect_async(url).await.unwrap();
    send(
        &mut ws,
        Up::Hello {
            version: PROTOCOL_VERSION,
            name: name.into(),
        },
    )
    .await;
    assert!(matches!(recv(&mut ws).await, Down::Welcome { .. }));
    ws
}

async fn send(ws: &mut Ws, up: Up) {
    ws.send(Message::binary(up.encode())).await.unwrap();
}

async fn recv(ws: &mut Ws) -> Down {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), ws.next())
            .await
            .expect("a frame within 5 s")
            .expect("the socket open")
            .expect("a good frame");
        match frame {
            Message::Binary(b) => return Down::decode(&b).expect("a Down"),
            Message::Ping(_) | Message::Pong(_) => continue,
            other => panic!("unexpected frame {other:?}"),
        }
    }
}

/// Skip lobby views until `pred` matches.
async fn until(ws: &mut Ws, pred: impl Fn(&Down) -> bool) -> Down {
    loop {
        let d = recv(ws).await;
        if pred(&d) {
            return d;
        }
    }
}

#[tokio::test]
async fn create_join_start_relay_and_close() {
    let (url, server) = serve().await;
    let mut host = connect(&url, "Host").await;
    let mut guest = connect(&url, "Guest").await;

    send(
        &mut host,
        Up::Create {
            settings: Settings::default(),
            loadout: Loadout::default(),
        },
    )
    .await;
    let Down::Lobby(view) = recv(&mut host).await else {
        panic!("expected the new lobby")
    };
    assert!(view.is_host());

    send(
        &mut guest,
        Up::Join {
            code: view.code.to_lowercase(),
            loadout: Loadout {
                class: 7,
                ..Loadout::default()
            },
        },
    )
    .await;
    let Down::Lobby(gv) = recv(&mut guest).await else {
        panic!("expected the joined lobby")
    };
    assert_eq!(gv.you, 1);
    assert_eq!(gv.members[1].loadout.class, 7);
    let Down::Lobby(hv) = recv(&mut host).await else {
        panic!("the host sees the guest arrive")
    };
    assert_eq!(hv.members.len(), 2);

    send(&mut guest, Up::SetReady(true)).await;
    until(&mut host, |d| matches!(d, Down::Lobby(v) if v.all_ready())).await;
    send(&mut host, Up::Start).await;
    let Down::Started { seed, .. } = until(&mut guest, |d| matches!(d, Down::Started { .. })).await
    else {
        unreachable!()
    };
    let Down::Started { seed: s2, .. } =
        until(&mut host, |d| matches!(d, Down::Started { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(seed, s2);

    // Guest → host: a batch arrives intact and stamped with the guest's seat.
    let batch = encode_batch(&[Game::Loaded]);
    send(
        &mut guest,
        Up::Relay {
            to: Target::Host,
            data: batch.clone(),
        },
    )
    .await;
    let Down::Relay { from, data } = until(&mut host, |d| matches!(d, Down::Relay { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(from, 1);
    assert_eq!(decode_batch(&data), vec![Game::Loaded]);

    // A garbage frame is answered, not fatal.
    host.send(Message::binary(vec![255u8, 1, 2])).await.unwrap();
    assert!(matches!(
        until(&mut host, |d| matches!(d, Down::Error(_))).await,
        Down::Error(_)
    ));

    // The host drops: the guest is told the lobby is gone.
    host.close(None).await.unwrap();
    drop(host);
    assert_eq!(
        until(&mut guest, |d| matches!(d, Down::Left(_))).await,
        Down::Left(LeftReason::HostLeft)
    );
    // The lobby is gone from the server.
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(server.counts().0, 0);
}

#[tokio::test]
async fn health_reports_counts() {
    let (url, _server) = serve().await;
    let _a = connect(&url, "a").await;
    let http = url.replace("ws://", "").replace("/lobby", "");
    let mut stream = tokio::net::TcpStream::connect(&http).await.unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream
        .write_all(
            format!("GET /lobby/health HTTP/1.1\r\nHost: {http}\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut body = String::new();
    stream.read_to_string(&mut body).await.unwrap();
    assert!(body.contains("ok protocol="), "{body}");
    assert!(body.contains("connections=1"), "{body}");
}

#[tokio::test]
async fn a_guest_dropped_in_a_run_comes_back_on_a_new_socket() {
    let (url, _server) = serve().await;
    let mut host = connect(&url, "Host").await;
    let mut guest = connect(&url, "Guest").await;
    send(
        &mut host,
        Up::Create {
            settings: Settings::default(),
            loadout: Loadout::default(),
        },
    )
    .await;
    let Down::Lobby(view) = recv(&mut host).await else {
        panic!("expected the new lobby")
    };
    let code = view.code.clone();
    send(
        &mut guest,
        Up::Join {
            code: code.clone(),
            loadout: Loadout::default(),
        },
    )
    .await;
    send(&mut guest, Up::SetReady(true)).await;
    until(&mut host, |d| matches!(d, Down::Lobby(v) if v.all_ready())).await;
    send(&mut host, Up::Start).await;
    let Down::Started { token, .. } =
        until(&mut guest, |d| matches!(d, Down::Started { .. })).await
    else {
        unreachable!()
    };

    // The guest's connection dies: the host sees its seat kept, away.
    drop(guest);
    let Down::Lobby(v) = until(
        &mut host,
        |d| matches!(d, Down::Lobby(v) if v.members.iter().any(|m| m.away)),
    )
    .await
    else {
        unreachable!()
    };
    assert_eq!(v.members.len(), 2);

    // A new socket, the token: the seat is back, and the run's relays flow to it again.
    let mut back = connect(&url, "Guest").await;
    send(&mut back, Up::Rejoin { code, token }).await;
    assert_eq!(
        until(&mut back, |d| matches!(d, Down::Rejoined | Down::Error(_))).await,
        Down::Rejoined
    );
    until(
        &mut host,
        |d| matches!(d, Down::Lobby(v) if v.members.len() == 2 && v.members.iter().all(|m| !m.away)),
    )
    .await;
    send(
        &mut host,
        Up::Relay {
            to: Target::Slot(1),
            data: encode_batch(&[Game::Resync]),
        },
    )
    .await;
    let Down::Relay { from, data } = until(&mut back, |d| matches!(d, Down::Relay { .. })).await
    else {
        unreachable!()
    };
    assert_eq!(from, 0);
    assert_eq!(decode_batch(&data), vec![Game::Resync]);
}
