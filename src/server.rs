use std::collections::VecDeque;
use std::fmt::Display;

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
use websocket::server::upgrade::WsUpgrade;
use websocket::sync::server::upgrade::Buffer;
use websocket::sync::server::HyperIntoWsError;
use websocket::sync::{Reader, Writer};
use websocket::{
    server::{NoTlsAcceptor, WsServer},
    sync::Server,
    OwnedMessage,
};

use crate::events::*;
use crate::writer::WebSocketWriter;

#[derive(Resource, Clone)]
pub struct WebSocketServerConfig {
    pub addr: SocketAddr,
    pub parsed_protocol: String,
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

type RequestQueueInner = Arc<Mutex<VecDeque<WsUpgrade<TcpStream, Option<Buffer>>>>>;

#[derive(Resource, Default, Deref)]
struct RequestQueue(RequestQueueInner);

struct Client {
    sender: Writer<TcpStream>,
    receiver: Reader<TcpStream>,
    mode: WebSocketClientMode,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum WebSocketClientMode {
    Parsed,
    Raw,
}

#[derive(Resource, Default)]
pub struct WebSocketClients {
    iter_index: usize,
    inner: IndexMap<WebSocketPeer, Client>,
}
impl WebSocketClients {
    pub fn write(&mut self, target: &WebSocketPeer) -> Option<WebSocketWriter> {
        self.inner.get_mut(target).map(|client| WebSocketWriter {
            sender: &mut client.sender,
        })
    }

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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Deref, DerefMut)]
pub struct WebSocketPeer(SocketAddr);
impl WebSocketPeer {
    pub fn write<'c>(&self, clients: &'c mut WebSocketClients) -> Option<WebSocketWriter<'c>> {
        clients.write(self)
    }

    pub fn set_mode(
        &self,
        clients: &mut WebSocketClients,
        mode: WebSocketClientMode,
    ) -> Option<()> {
        clients.set_mode(self, mode)
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

fn start_server(
    config: WebSocketServerConfig,
) -> Result<WsServer<NoTlsAcceptor, TcpListener>, io::Error> {
    let server = Server::bind(config.addr)?;
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

    for request in server.into_iter() {
        match request {
            Ok(req) => queue.lock_arc().push_back(req),
            Err(e) => {
                if let HyperIntoWsError::Io(ref e) = e.error {
                    if e.kind() == io::ErrorKind::WouldBlock {
                        thread::sleep(Duration::from_millis(50));
                    }
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
            let protocols = request.protocols();
            let (mode, protocol) = if protocols.contains(&config.parsed_protocol) {
                (WebSocketClientMode::Parsed, &config.parsed_protocol)
            } else if protocols.contains(&config.raw_protocol) {
                (WebSocketClientMode::Raw, &config.raw_protocol)
            } else {
                request.reject().map_err(|(_, e)| e)?;
                return Ok(());
            };

            let client = request
                .use_protocol(protocol)
                .accept()
                .map_err(|(_, e)| e)?;

            let peer = WebSocketPeer(client.peer_addr()?);
            info!("New connection from: {}", peer);
            let (receiver, sender) = client.split()?;

            open_w.send(WebSocketOpenEvent { peer, mode });

            clients.inner.insert(
                peer,
                Client {
                    sender,
                    receiver,
                    mode,
                },
            );
        }
    }

    Ok(())
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
    mut data_w: EventWriter<WebSocketRawEvent>,
    mut close_w: EventWriter<WebSocketCloseEvent>,
) {
    if let Some((peer, client)) = clients.next() {
        let peer = *peer;

        match client.mode {
            WebSocketClientMode::Parsed => {
                if let Ok(msg) = client.receiver.recv_message() {
                    match msg {
                        OwnedMessage::Text(data) => {
                            message_w.send(WebSocketMessageEvent { data, peer });
                        }
                        OwnedMessage::Binary(data) => {
                            binary_w.send(WebSocketBinaryEvent { data, peer });
                        }
                        OwnedMessage::Ping(data) => {
                            if client
                                .sender
                                .send_message(&OwnedMessage::Pong(data))
                                .is_err()
                            {
                                error!("Failed to reply to ping.");
                            }
                        }
                        OwnedMessage::Pong(data) => {
                            pong_w.send(WebSocketPongEvent { data, peer });
                        }
                        OwnedMessage::Close(data) => {
                            clients.inner.swap_remove(&peer);

                            close_w.send(WebSocketCloseEvent { data, peer });
                        }
                    };
                }
            }
            WebSocketClientMode::Raw => {
                if let Ok(data) = client.receiver.recv_dataframe() {
                    data_w.send(WebSocketRawEvent { data, peer });
                }
            }
        }
    }
}
