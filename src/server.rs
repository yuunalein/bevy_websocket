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
    pub protocol: String,
}
impl Default for WebSocketServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 0)),
            protocol: "bevy_websocket".to_string(),
        }
    }
}

type RequestQueueInner = Arc<Mutex<VecDeque<WsUpgrade<TcpStream, Option<Buffer>>>>>;

#[derive(Resource, Default, Deref)]
struct RequestQueue(RequestQueueInner);

struct Client {
    sender: Writer<TcpStream>,
    receiver: Reader<TcpStream>,
}

#[derive(Resource, Default)]
pub struct WebSocketClients {
    iter_index: usize,
    inner: IndexMap<WebSocketPeer, Client>,
}
impl WebSocketClients {
    pub fn write(&mut self, target: &WebSocketPeer) -> Option<WebSocketWriter> {
        self.inner.get_mut(target).map(|req| WebSocketWriter {
            sender: &mut req.sender,
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
    mut requests: ResMut<WebSocketClients>,
    config: Res<WebSocketServerConfig>,
    mut open_w: EventWriter<WebSocketOpenEvent>,
) -> Result<(), io::Error> {
    if !request_queue.0.is_locked() {
        let mut queue = request_queue.clone().lock_arc();
        if let Some(request) = queue.pop_front() {
            if !request.protocols().contains(&config.protocol) {
                request.reject().map_err(|(_, e)| e)?;
                return Ok(());
            }

            let client = request
                .use_protocol(&config.protocol)
                .accept()
                .map_err(|(_, e)| e)?;

            let peer = WebSocketPeer(client.peer_addr()?);
            info!("New connection from: {}", peer);
            let (receiver, sender) = client.split()?;

            open_w.send(WebSocketOpenEvent { peer });

            requests.inner.insert(peer, Client { sender, receiver });
        }
    }

    Ok(())
}

fn handle_request(
    request_queue: Res<RequestQueue>,
    requests: ResMut<WebSocketClients>,
    config: Res<WebSocketServerConfig>,
    open_w: EventWriter<WebSocketOpenEvent>,
) {
    if let Err(error) = handle_request_inner(request_queue, requests, config, open_w) {
        error!("Failed to get request. - {error}");
    }
}

fn handle_client(
    mut requests: ResMut<WebSocketClients>,
    mut message_w: EventWriter<WebSocketMessageEvent>,
    mut binary_w: EventWriter<WebSocketBinaryEvent>,
    mut pong_w: EventWriter<WebSocketPongEvent>,
    mut close_w: EventWriter<WebSocketCloseEvent>,
) {
    if let Some((peer, request)) = requests.next() {
        if let Ok(msg) = request.receiver.recv_message() {
            let peer = *peer;

            match msg {
                OwnedMessage::Text(data) => {
                    message_w.send(WebSocketMessageEvent { data, peer });
                }
                OwnedMessage::Binary(data) => {
                    binary_w.send(WebSocketBinaryEvent { data, peer });
                }
                OwnedMessage::Ping(data) => {
                    if request
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
                    requests.inner.swap_remove(&peer);

                    close_w.send(WebSocketCloseEvent { data, peer });
                }
            };
        }
    }
}
