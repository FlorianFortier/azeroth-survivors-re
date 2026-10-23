//! The lobby server: [`survivors_net::hub::Hub`] behind a WebSocket at
//! [`survivors_net::protocol::LOBBY_PATH`]. Every rule lives in the hub; this crate only moves
//! frames, keeps each connection's outbound queue bounded, and drops connections that go quiet or
//! flood.
//!
//! [`router`] is what `wenilla-host` mounts beside the game; `main.rs` serves it alone.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use survivors_net::hub::{ConnId, Hub, Limits, Send};
use survivors_net::protocol::{Down, Up, LOBBY_PATH, MAX_FRAME};
use survivors_net::Wire;
use tokio::sync::mpsc;

/// How the server treats connections.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub limits: Limits,
    /// Connections served at once.
    pub max_connections: usize,
    /// Frames queued for one slow connection before it is dropped.
    pub queue: usize,
    /// A connection silent this long is closed. The server pings at a third of it.
    pub idle: Duration,
    /// Frames a connection may send per second, averaged over [`Config::burst`].
    pub frames_per_sec: u32,
    pub burst: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            max_connections: 4096,
            queue: 2048,
            idle: Duration::from_secs(60),
            // A host sends a batch per tick (20 Hz) plus lobby traffic; a guest less.
            frames_per_sec: 120,
            burst: 240,
        }
    }
}

struct Inner {
    hub: Mutex<Hub>,
    queues: Mutex<HashMap<ConnId, mpsc::Sender<Message>>>,
    next: AtomicU64,
    config: Config,
}

/// The server's shared state; cheap to clone.
#[derive(Clone)]
pub struct LobbyServer(Arc<Inner>);

impl LobbyServer {
    pub fn new(config: Config) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        Self(Arc::new(Inner {
            hub: Mutex::new(Hub::new(seed, config.limits)),
            queues: Mutex::new(HashMap::new()),
            next: AtomicU64::new(1),
            config,
        }))
    }

    /// `(lobbies, connections)`, for the health route and logs.
    pub fn counts(&self) -> (usize, usize) {
        let hub = self.0.hub.lock().expect("hub lock");
        (hub.lobby_count(), hub.connection_count())
    }

    fn dispatch(&self, sends: Vec<Send>) {
        if sends.is_empty() {
            return;
        }
        let queues = self.0.queues.lock().expect("queues lock");
        for s in sends {
            if let Some(q) = queues.get(&s.to) {
                // A full queue is a client that stopped reading: close it rather than buffer
                // without bound. Its reader sees the channel close and disconnects it.
                if q.try_send(Message::binary(s.msg.encode())).is_err() {
                    tracing::debug!(conn = s.to, "outbound queue full; dropping connection");
                    let _ = q.try_send(Message::Close(None));
                }
            }
        }
    }

    async fn serve(self, socket: WebSocket) {
        let config = self.0.config;
        let conn = self.0.next.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::channel::<Message>(config.queue);
        {
            let mut queues = self.0.queues.lock().expect("queues lock");
            if queues.len() >= config.max_connections {
                tracing::warn!("connection limit reached; refusing a connection");
                return;
            }
            queues.insert(conn, tx);
        }
        self.0.hub.lock().expect("hub lock").connect(conn);
        tracing::debug!(conn, "lobby connection open");

        let (mut ws_tx, mut ws_rx) = socket.split();
        let writer = tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                let close = matches!(m, Message::Close(_));
                if ws_tx.send(m).await.is_err() || close {
                    break;
                }
            }
            let _ = ws_tx.close().await;
        });

        let mut ping = tokio::time::interval(config.idle / 3);
        ping.tick().await;
        let mut last_heard = tokio::time::Instant::now();
        // A token bucket: `burst` frames, refilled at `frames_per_sec`.
        let mut tokens = config.burst as f64;
        let mut refilled = tokio::time::Instant::now();

        loop {
            tokio::select! {
                frame = ws_rx.next() => {
                    let Some(Ok(frame)) = frame else { break };
                    last_heard = tokio::time::Instant::now();
                    let now = last_heard;
                    tokens = (tokens
                        + now.duration_since(refilled).as_secs_f64() * f64::from(config.frames_per_sec))
                        .min(f64::from(config.burst));
                    refilled = now;
                    match frame {
                        Message::Binary(data) => {
                            if tokens < 1.0 {
                                tracing::debug!(conn, "frame rate exceeded; dropping connection");
                                break;
                            }
                            tokens -= 1.0;
                            let sends = if data.len() > MAX_FRAME {
                                self.0.hub.lock().expect("hub lock").bad_frame(conn)
                            } else {
                                match Up::decode(&data) {
                                    Ok(up) => self.0.hub.lock().expect("hub lock").handle(conn, up),
                                    Err(_) => self.0.hub.lock().expect("hub lock").bad_frame(conn),
                                }
                            };
                            self.dispatch(sends);
                        }
                        Message::Text(_) => {
                            let sends = self.0.hub.lock().expect("hub lock").bad_frame(conn);
                            self.dispatch(sends);
                        }
                        Message::Ping(_) | Message::Pong(_) => {}
                        Message::Close(_) => break,
                    }
                }
                _ = ping.tick() => {
                    if last_heard.elapsed() > config.idle {
                        tracing::debug!(conn, "idle; closing");
                        break;
                    }
                    let queues = self.0.queues.lock().expect("queues lock");
                    if let Some(q) = queues.get(&conn) {
                        let _ = q.try_send(Message::Ping(Vec::new().into()));
                    }
                }
            }
            if writer.is_finished() {
                break;
            }
        }

        let sends = self.0.hub.lock().expect("hub lock").disconnect(conn);
        self.dispatch(sends);
        if let Some(q) = self.0.queues.lock().expect("queues lock").remove(&conn) {
            let _ = q.try_send(Message::Close(None));
        }
        let _ = writer.await;
        tracing::debug!(conn, "lobby connection closed");
    }
}

/// The lobby at [`LOBBY_PATH`], and a plain-text health line at `{LOBBY_PATH}/health`.
pub fn router(server: LobbyServer) -> Router {
    Router::new()
        .route(LOBBY_PATH, get(upgrade))
        .route(&format!("{LOBBY_PATH}/health"), get(health))
        .with_state(server)
}

async fn upgrade(State(server): State<LobbyServer>, ws: WebSocketUpgrade) -> Response {
    ws.max_message_size(MAX_FRAME)
        .max_frame_size(MAX_FRAME)
        .on_upgrade(move |socket| server.serve(socket))
}

async fn health(State(server): State<LobbyServer>) -> impl IntoResponse {
    let (lobbies, conns) = server.counts();
    format!(
        "ok protocol={} lobbies={lobbies} connections={conns}\n",
        survivors_net::protocol::PROTOCOL_VERSION
    )
}

/// Turn a [`Down`] frame back into its message (for tests and tools).
pub fn decode_down(data: &[u8]) -> Option<Down> {
    Down::decode(data).ok()
}
