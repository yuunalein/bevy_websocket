use std::collections::VecDeque;
use std::ops::Deref;

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

#[derive(Resource, Default)]
struct RequestQueue(RequestQueueInner);
impl Deref for RequestQueue {
    type Target = RequestQueueInner;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

struct Client {
    sender: Writer<TcpStream>,
    receiver: Reader<TcpStream>,
}

#[derive(Resource, Default)]
pub struct Clients {
    iter_index: usize,
    inner: IndexMap<SocketAddr, Client>,
}
impl Clients {
    pub fn write(&mut self, target: &SocketAddr) -> Option<WebSocketWriter> {
        self.inner.get_mut(target).map(|req| WebSocketWriter {
            sender: &mut req.sender,
        })
    }
}
impl Clients {
    fn next(&mut self) -> Option<(&SocketAddr, &mut Client)> {
        if self.inner.is_empty() {
            return None;
        }

        self.iter_index = (self.iter_index + 1) % self.inner.len();
        self.inner.get_index_mut(self.iter_index)
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
        .init_resource::<Clients>()
        .add_event::<WebSocketMessage>()
        .add_event::<WebSocketBinary>()
        .add_event::<WebSocketOpen>()
        .add_event::<WebSocketClose>()
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
    mut requests: ResMut<Clients>,
    config: Res<WebSocketServerConfig>,
    mut open_w: EventWriter<WebSocketOpen>,
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

            let peer = client.peer_addr()?;
            info!("New connection from: {}", peer);
            let (receiver, sender) = client.split()?;

            open_w.send(WebSocketOpen { peer });

            requests.inner.insert(peer, Client { sender, receiver });
        }
    }

    Ok(())
}

fn handle_request(
    request_queue: Res<RequestQueue>,
    requests: ResMut<Clients>,
    config: Res<WebSocketServerConfig>,
    open_w: EventWriter<WebSocketOpen>,
) {
    if let Err(error) = handle_request_inner(request_queue, requests, config, open_w) {
        error!("Failed to get request. - {error}");
    }
}

fn handle_client(
    mut requests: ResMut<Clients>,
    mut message_w: EventWriter<WebSocketMessage>,
    mut binary_w: EventWriter<WebSocketBinary>,
    mut close_w: EventWriter<WebSocketClose>,
) {
    if let Some((peer, request)) = requests.next() {
        if let Ok(msg) = request.receiver.recv_message() {
            let peer = *peer;

            match msg {
                OwnedMessage::Text(data) => {
                    message_w.send(WebSocketMessage { data, peer });
                }
                OwnedMessage::Binary(data) => {
                    binary_w.send(WebSocketBinary { data, peer });
                }
                OwnedMessage::Close(data) => {
                    close_w.send(WebSocketClose { data, peer });

                    requests.inner.swap_remove(&peer);
                }
                OwnedMessage::Ping(ping) => {
                    if request
                        .sender
                        .send_message(&OwnedMessage::Pong(ping))
                        .is_err()
                    {
                        error!("Failed to reply to ping.");
                    }
                }
                _ => (),
            };
        }
    }
}
