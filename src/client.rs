use std::net::TcpStream;

use bevy::prelude::*;
use indexmap::IndexMap;
use tungstenite::WebSocket;

use crate::{peer::WebSocketPeer, writer::WebSocketWriter};

#[derive(Debug)]
pub(crate) struct Client {
    pub stream: WebSocket<TcpStream>,
    pub mode: WebSocketClientMode,
}

/// A client can operate in either Parsed or Raw mode.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum WebSocketClientMode {
    Parsed,
    Raw,
}

/// A map of active web-socket clients.
///
/// ```
/// fn send(mut clients: ResMut<WebSocketClients>) {
///     clients
///         .write(&"127.0.0.1:42069".parse().unwrap())
///         .unwrap()
///         .send_message("Hello World")
///         .unwrap();
/// }
/// ```
#[derive(Resource, Default)]
pub struct WebSocketClients {
    iter_index: usize,
    pub(crate) inner: IndexMap<WebSocketPeer, Client>,
}
impl WebSocketClients {
    /// Create a [`WebSocketWriter`] for a client.
    ///
    /// Returns [None] if a client with the specified [`WebSocketPeer`] does not exist.
    pub fn write(&mut self, target: &WebSocketPeer) -> Option<WebSocketWriter> {
        self.inner.get_mut(target).map(|client| WebSocketWriter {
            stream: &mut client.stream,
        })
    }

    /// Set the operation mode for a client.
    ///
    /// Returns [None] if a client with the specified [`WebSocketPeer`] does not exist.
    pub fn set_mode(&mut self, target: &WebSocketPeer, mode: WebSocketClientMode) -> Option<()> {
        self.inner.get_mut(target).map(|client| {
            client.mode = mode;
        })
    }

    pub(crate) fn next(&mut self) -> Option<(&WebSocketPeer, &mut Client)> {
        if self.inner.is_empty() {
            return None;
        }

        self.iter_index = (self.iter_index + 1) % self.inner.len();
        self.inner.get_index_mut(self.iter_index)
    }
}
