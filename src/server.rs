use std::{
    collections::VecDeque,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    sync::Arc,
};

use bevy::{
    prelude::*,
    tasks::{
        futures_lite::future::{self, yield_now},
        AsyncComputeTaskPool,
    },
};
use parking_lot::Mutex;
use websocket::{
    server::{upgrade::WsUpgrade, NoTlsAcceptor, WsServer},
    sync::{server::upgrade::Buffer, Server},
    OwnedMessage, WebSocketError,
};

use crate::{events::*, writer::SenderMap};

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

#[derive(Resource)]
pub struct WebSocketServer {
    config: Arc<WebSocketServerConfig>,
    pub queue: EventQueue,
    pub sender_map: SenderMap,
}
impl WebSocketServer {
    pub fn new(config: WebSocketServerConfig, sender_map: SenderMap) -> Self {
        Self {
            config: Arc::new(config),
            sender_map,
            queue: Default::default(),
        }
    }

    pub fn run(&mut self) -> io::Result<()> {
        let server = Server::bind(self.config.addr)?;
        info!("Server running at ws://{}", self.config.addr);
        let thread_pool = AsyncComputeTaskPool::get();

        // Spawn a new thread since the server has to run in background.
        let config = self.config.clone();
        let queue = self.queue.clone();
        let sender_map = self.sender_map.clone();
        thread_pool
            .spawn(serve(server, config, queue, sender_map))
            .detach();

        Ok(())
    }
}

async fn serve(
    server: WsServer<NoTlsAcceptor, TcpListener>,
    config: Arc<WebSocketServerConfig>,
    queue: EventQueue,
    sender_map: SenderMap,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    let mut requests = server.filter_map(Result::ok);
    loop {
        if let Some(request) = requests.next() {
            let config = config.clone();
            let queue = queue.clone();
            let sender_map = sender_map.clone();
            let mut handle = thread_pool.spawn(handle_request(request, config, queue, sender_map));

            thread_pool
                .spawn(async move {
                    loop {
                        if future::poll_once(&mut handle).await.is_some() {
                            break;
                        }

                        yield_now().await;
                    }
                })
                .detach();
        }

        yield_now().await;
    }
}

async fn handle_request(
    request: WsUpgrade<TcpStream, Option<Buffer>>,
    config: Arc<WebSocketServerConfig>,
    queue: EventQueue,
    sender_map: SenderMap,
) {
    match request.tcp_stream().peer_addr() {
        Ok(peer) => {
            handle_request_inner(request, config, queue.clone(), sender_map.clone())
                .await
                .unwrap_or_else(|error| {
                    error!("WebSocket request handler execution failed - {}", error);
                    queue
                        .lock_arc()
                        .push_back(EventKind::Close(WebSocketClose { data: None, peer }));
                    sender_map.lock_arc().remove(&peer);
                });
        }
        Err(error) => error!("Failed to establish websocket stream - {error}"),
    };
}

async fn handle_request_inner(
    request: WsUpgrade<TcpStream, Option<Buffer>>,
    config: Arc<WebSocketServerConfig>,
    queue: EventQueue,
    sender_map: SenderMap,
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
    let (mut receiver, sender) = client.split()?;
    let sender = Arc::new(Mutex::new(sender));

    sender_map.lock_arc().insert(peer, sender.clone());
    queue
        .lock_arc()
        .push_back(EventKind::Open(WebSocketOpen { peer }));

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
                sender_map.lock_arc().remove(&peer);
                return Ok(());
            }
            OwnedMessage::Ping(ping) => sender.lock().send_message(&OwnedMessage::Pong(ping))?,
            _ => (),
        };
    }

    Ok(())
}
