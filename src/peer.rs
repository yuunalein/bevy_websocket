use std::{
    fmt::Display,
    net::{AddrParseError, SocketAddr},
    str::FromStr,
};

use bevy::prelude::*;

use crate::{
    client::{WebSocketClientMode, WebSocketClients},
    writer::WebSocketWriter,
};

/// Used to identify clients in [`WebSocketClients`].
///
/// Wraps a [SocketAddr].
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Deref, DerefMut)]
pub struct WebSocketPeer(pub SocketAddr);
impl WebSocketPeer {
    /// Create a [`WebSocketWriter`] for the client corresponding to this [`WebSocketPeer`].
    ///
    /// Returns [None] if a client with this [`WebSocketPeer`] does not exist.
    pub fn write<'c>(&self, clients: &'c mut WebSocketClients) -> Option<WebSocketWriter<'c>> {
        clients.write(self)
    }

    /// Set the operation mode for the client corresponding to this [`WebSocketPeer`].
    ///
    /// Returns [None] if a client with this [`WebSocketPeer`] does not exist.
    pub fn set_mode(
        &self,
        clients: &mut WebSocketClients,
        mode: WebSocketClientMode,
    ) -> Option<()> {
        clients.set_mode(self, mode)
    }
}
impl FromStr for WebSocketPeer {
    type Err = AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(SocketAddr::from_str(s)?))
    }
}
impl Display for WebSocketPeer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
