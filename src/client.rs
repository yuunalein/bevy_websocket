use std::net::TcpStream;

use bevy::prelude::*;
use indexmap::IndexMap;
use tungstenite::{
    client::IntoClientRequest, connect, http::Response, protocol::frame::FrameSocket,
    stream::MaybeTlsStream, Error, Message, WebSocket,
};

use crate::{events::*, peer::WebSocketPeer, writer::WebSocketWriter};

#[derive(Debug)]
pub(crate) struct Client {
    pub stream: WebSocket<MaybeTlsStream<TcpStream>>,
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
    #[allow(clippy::type_complexity)]
    pub fn request<Req: IntoClientRequest>(
        &mut self,
        request: Req,
        mode: WebSocketClientMode,
    ) -> Result<(WebSocketPeer, Response<Option<Vec<u8>>>), Error> {
        let (stream, response) = connect(request)?;
        let peer = WebSocketPeer::from_maybe_tls_stream(stream.get_ref())?;

        self.inner.insert(peer, Client { stream, mode });
        Ok((peer, response))
    }

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

pub(crate) fn handle_clients(
    mut clients: ResMut<WebSocketClients>,
    mut message_w: EventWriter<WebSocketMessageEvent>,
    mut binary_w: EventWriter<WebSocketBinaryEvent>,
    mut pong_w: EventWriter<WebSocketPongEvent>,
    mut raw_w: EventWriter<WebSocketRawEvent>,
    mut close_w: EventWriter<WebSocketCloseEvent>,
) {
    if let Some((peer, client)) = clients.next() {
        let peer = *peer;

        match client.mode {
            WebSocketClientMode::Parsed => {
                if let Ok(msg) = client.stream.read() {
                    match msg {
                        Message::Text(data) => {
                            message_w.send(WebSocketMessageEvent {
                                data: data.to_string(),
                                peer,
                            });
                        }
                        Message::Binary(data) => {
                            binary_w.send(WebSocketBinaryEvent { data, peer });
                        }
                        Message::Ping(data) => {
                            if client.stream.send(Message::Pong(data)).is_err() {
                                error!("Failed to reply to ping.");
                            }
                        }
                        Message::Pong(data) => {
                            pong_w.send(WebSocketPongEvent { data, peer });
                        }
                        Message::Close(data) => {
                            clients.inner.swap_remove(&peer);

                            close_w.send(WebSocketCloseEvent { data, peer });
                        }
                        _ => (),
                    };
                }
            }
            WebSocketClientMode::Raw => {
                let max_size = client.stream.get_config().max_frame_size;
                let mut reader = FrameSocket::new(client.stream.get_mut());

                if let Ok(Some(data)) = reader.read(max_size) {
                    raw_w.send(WebSocketRawEvent { data, peer });
                }
            }
        }
    }
}
