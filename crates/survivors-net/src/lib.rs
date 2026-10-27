//! Azeroth Survivors' multiplayer, minus the I/O.
//!
//! - [`wire`]: the byte codec every message uses.
//! - [`settings`]: what a lobby's host tunes, and its bounds.
//! - [`protocol`]: game ↔ lobby server messages ([`protocol::Up`], [`protocol::Down`]).
//! - [`game`]: host ↔ client messages inside a run, relayed by the server untouched.
//! - [`hub`]: the lobby server's state machine — codes, seats, readiness, the run, the relay.
//!
//! The server (`survivors-server`, also mounted by `wenilla-host`) wraps [`hub::Hub`] in
//! WebSockets; the game (`benilla-app`'s survivors mode) speaks [`protocol`] and [`game`].

pub mod game;
pub mod hub;
pub mod protocol;
pub mod settings;
pub mod wire;

pub use wire::Wire;

#[cfg(test)]
mod fuzz;
