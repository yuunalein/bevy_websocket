use std::collections::VecDeque;
use std::fmt::Display;
use std::mem::MaybeUninit;
use std::net::AddrParseError;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::{
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
};

use bevy::prelude::*;
use indexmap::IndexMap;
use parking_lot::Mutex;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::{HeaderMap, HeaderValue, StatusCode};
use tungstenite::protocol::frame::FrameSocket;
use tungstenite::{accept_hdr, Message, WebSocket};

use crate::events::*;
use crate::writer::WebSocketWriter;

#[derive(Resource, Clone)]
pub struct WebSocketServerConfig {
    /// Address which the server will listen on.
    pub addr: SocketAddr,

    /// Protocol used for conversations that will be parsed inside this crate.
    /// (Message, Binary, Ping, Pong, Close)
    pub parsed_protocol: String,

    /// Protocol used for raw conversations.
    pub raw_protocol: String,
}
impl Default for WebSocketServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0)),
            parsed_protocol: "bevy_websocket".to_string(),
            raw_protocol: "bevy_websocket_raw".to_string(),
        }
    }
}

type RequestQueueInner = Arc<Mutex<VecDeque<TcpStream>>>;

#[derive(Resource, Default, Deref)]
struct RequestQueue(RequestQueueInner);

#[derive(Debug)]
pub struct Client {
    stream: WebSocket<TcpStream>,
    mode: WebSocketClientMode,
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
    inner: IndexMap<WebSocketPeer, Client>,
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

    fn next(&mut self) -> Option<(&WebSocketPeer, &mut Client)> {
        if self.inner.is_empty() {
            return None;
        }

        self.iter_index = (self.iter_index + 1) % self.inner.len();
        self.inner.get_index_mut(self.iter_index)
    }
}

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

pub(crate) fn install_websocket_server(app: &mut App, config: WebSocketServerConfig) -> &mut App {
    let queue = RequestQueue::default();

    {
        let queue = queue.clone();
        let config = config.clone();

        thread::spawn(move || listen(config, queue));
    }

    app.insert_resource(config)
        .insert_resource(queue)
        .init_resource::<WebSocketClients>()
        .add_event::<WebSocketMessageEvent>()
        .add_event::<WebSocketBinaryEvent>()
        .add_event::<WebSocketPongEvent>()
        .add_event::<WebSocketRawEvent>()
        .add_event::<WebSocketOpenEvent>()
        .add_event::<WebSocketCloseEvent>()
        .add_systems(Update, (handle_request, handle_client))
}

fn start_server(config: WebSocketServerConfig) -> Result<TcpListener, io::Error> {
    let server = TcpListener::bind(config.addr)?;
    info!("Server running at ws://{}", server.local_addr()?);
    server.set_nonblocking(true)?;

    Ok(server)
}

fn listen(config: WebSocketServerConfig, queue: RequestQueueInner) {
    let server = match start_server(config) {
        Ok(server) => server,
        Err(error) => {
            error!("Failed to start websocket server. - {}", error);
            return;
        }
    };

    for request in server.incoming() {
        match request {
            Ok(req) => queue.lock_arc().push_back(req),
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    thread::sleep(Duration::from_millis(50));
                }
            }
        };
    }
}

fn handle_request_inner(
    request_queue: Res<RequestQueue>,
    mut clients: ResMut<WebSocketClients>,
    config: Res<WebSocketServerConfig>,
    mut open_w: EventWriter<WebSocketOpenEvent>,
) -> Result<(), io::Error> {
    if !request_queue.0.is_locked() {
        let mut queue = request_queue.clone().lock_arc();
        if let Some(request) = queue.pop_front() {
            let peer = request.peer_addr()?;
            let mut mode: MaybeUninit<WebSocketClientMode> = MaybeUninit::uninit();
            let mut headers: MaybeUninit<HeaderMap<HeaderValue>> = MaybeUninit::uninit();

            if let Ok(stream) = accept_hdr(request, |request: &Request, response: Response| {
                handle_accept(request, response, &config, &mut mode, &mut headers)
            }) {
                let peer = WebSocketPeer(peer);
                info!("New connection from: {}", peer);

                let (mode, headers) = unsafe { (mode.assume_init(), headers.assume_init()) };

                clients.inner.insert(peer, Client { stream, mode });

                open_w.send(WebSocketOpenEvent {
                    peer,
                    mode,
                    headers,
                });
            }
        }
    }

    Ok(())
}

#[allow(clippy::result_large_err)]
fn handle_accept(
    request: &Request,
    mut response: Response,
    config: &WebSocketServerConfig,
    mode: &mut MaybeUninit<WebSocketClientMode>,
    headers: &mut MaybeUninit<HeaderMap<HeaderValue>>,
) -> Result<Response, ErrorResponse> {
    headers.write(request.headers().clone());

    if let Some(protocols) = request.headers().get("Sec-WebSocket-Protocol") {
        let protocols: Vec<&str> = protocols
            .to_str()
            .unwrap_or("")
            .split(',')
            .map(|item| item.trim())
            .collect();

        if protocols.contains(&config.parsed_protocol.as_str()) {
            mode.write(WebSocketClientMode::Parsed);

            response.headers_mut().append(
                "Sec-WebSocket-Protocol",
                config
                    .parsed_protocol
                    .parse()
                    .expect("Failed to parse protocol"),
            );
            Ok(response)
        } else if protocols.contains(&config.raw_protocol.as_str()) {
            mode.write(WebSocketClientMode::Raw);

            response.headers_mut().append(
                "Sec-WebSocket-Protocol",
                config
                    .raw_protocol
                    .parse()
                    .expect("Failed to parse protocol"),
            );

            Ok(response)
        } else {
            Err(Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(None)
                .expect("Failed to build error response."))
        }
    } else {
        Err(Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body(None)
            .expect("Failed to build error response."))
    }
}

fn handle_request(
    request_queue: Res<RequestQueue>,
    clients: ResMut<WebSocketClients>,
    config: Res<WebSocketServerConfig>,
    open_w: EventWriter<WebSocketOpenEvent>,
) {
    if let Err(error) = handle_request_inner(request_queue, clients, config, open_w) {
        error!("Failed to get request. - {error}");
    }
}

fn handle_client(
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
