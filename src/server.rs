use std::{
    collections::VecDeque,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    sync::Arc,
};

use bevy::{prelude::*, tasks::AsyncComputeTaskPool};
use parking_lot::Mutex;
use websocket::{
    server::{upgrade::WsUpgrade, NoTlsAcceptor, WsServer},
    sync::{server::upgrade::Buffer, Server},
    OwnedMessage, WebSocketError,
};

use crate::events::*;

#[derive(Debug, Clone)]
pub struct WebSocketServerConfig {
    pub addr: SocketAddr,
    pub protocol: String,
}
impl Default for WebSocketServerConfig {
    fn default() -> Self {
        Self {
            addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 2794)),
            protocol: "bevy_websocket".to_string(),
        }
    }
}

type EventQueue = Arc<Mutex<VecDeque<EventKind>>>;

#[derive(Debug, Resource, Default)]
pub struct WebSocketServer {
    config: Arc<WebSocketServerConfig>,
    pub queue: EventQueue,
}
impl WebSocketServer {
    pub fn new(config: WebSocketServerConfig) -> Self {
        Self {
            config: Arc::new(config),
            ..Default::default()
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        let server = Server::bind(self.config.addr)?;
        info!("Server running at ws://{}", self.config.addr);
        let thread_pool = AsyncComputeTaskPool::get();

        // Spawn a new thread since the server has to run in background.
        let config = self.config.clone();
        let queue = self.queue.clone();
        thread_pool.spawn(serve(server, config, queue)).detach();

        Ok(())
    }
}

async fn serve(
    server: WsServer<NoTlsAcceptor, TcpListener>,
    config: Arc<WebSocketServerConfig>,
    queue: EventQueue,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for request in server.filter_map(Result::ok) {
        let config = config.clone();
        let queue = queue.clone();
        thread_pool
            .spawn(handle_request(request, config, queue))
            .detach();
    }
}

async fn handle_request(
    request: WsUpgrade<TcpStream, Option<Buffer>>,
    config: Arc<WebSocketServerConfig>,
    queue: EventQueue,
) {
    match request.tcp_stream().peer_addr() {
        Ok(peer) => {
            handle_request_inner(request, config, queue.clone())
                .await
                .unwrap_or_else(|error| {
                    error!("WebSocket request handler execution failed - {}", error);
                    queue
                        .lock_arc()
                        .push_back(EventKind::Close(WebSocketClose { data: None, peer }));
                });
        }
        Err(error) => error!("Failed to establish websocket stream - {error}"),
    };
}

async fn handle_request_inner(
    request: WsUpgrade<TcpStream, Option<Buffer>>,
    config: Arc<WebSocketServerConfig>,
    queue: EventQueue,
) -> Result<(), WebSocketError> {
    if !request.protocols().contains(&config.protocol) {
        request.reject().map_err(|(_, e)| e)?;
        return Ok(());
    }

    let client = request
        .use_protocol(&config.protocol)
        .accept()
        .map_err(|(_, e)| e)?;

    let peer = client.peer_addr()?;
    info!("New connection from: {peer}");
    queue
        .lock_arc()
        .push_back(EventKind::Open(WebSocketOpen { peer }));

    let (mut receiver, mut sender) = client.split()?;

    for msg in receiver.incoming_messages() {
        let msg = msg?;

        match msg {
            OwnedMessage::Text(data) => queue
                .lock_arc()
                .push_back(EventKind::Message(WebSocketMessage { data, peer })),
            OwnedMessage::Binary(data) => queue
                .lock_arc()
                .push_back(EventKind::Binary(WebSocketBinary { data, peer })),
            OwnedMessage::Close(data) => {
                queue
                    .lock_arc()
                    .push_back(EventKind::Close(WebSocketClose { data, peer }));
                return Ok(());
            }
            OwnedMessage::Ping(ping) => sender.send_message(&OwnedMessage::Pong(ping))?,
            _ => (),
        };
    }

    Ok(())
}
