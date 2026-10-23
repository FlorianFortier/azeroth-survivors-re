//! `survivors-server`: the lobby server on its own, for a box that hosts lobbies for players who
//! run the game elsewhere (their own `wenilla-host`, or the native build). It serves no game
//! files, so it is safe to expose; put it behind a TLS proxy for players on an `https` page.

use anyhow::{Context, Result};
use clap::Parser;
use survivors_server::{router, Config, LobbyServer};

#[derive(Parser)]
#[command(about = "Azeroth Survivors' self-hosted lobby server (lobbies by code, run relay)")]
struct Cli {
    /// Address to listen on.
    #[arg(long, default_value = "0.0.0.0:7777")]
    bind: String,
    /// Lobbies the server holds at once.
    #[arg(long, default_value_t = 1024)]
    max_lobbies: usize,
    /// Connections served at once.
    #[arg(long, default_value_t = 4096)]
    max_connections: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let mut config = Config::default();
    config.limits.max_lobbies = cli.max_lobbies;
    config.max_connections = cli.max_connections;
    let app = router(LobbyServer::new(config));
    let listener = tokio::net::TcpListener::bind(&cli.bind)
        .await
        .with_context(|| format!("binding {}", cli.bind))?;
    tracing::info!(bind = %cli.bind, path = survivors_net::protocol::LOBBY_PATH, "lobby server listening");
    axum::serve(listener, app).await.context("serving")
}
