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

#[derive(Debug, Resource, Default)]
pub struct WebSocketServer {
    config: Arc<WebSocketServerConfig>,
    pub msg_queue: Arc<Mutex<VecDeque<WebSocketMessage>>>,
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
        let msg_queue = self.msg_queue.clone();
        thread_pool.spawn(serve(server, config, msg_queue)).detach();

        Ok(())
    }
}

async fn serve(
    server: WsServer<NoTlsAcceptor, TcpListener>,
    config: Arc<WebSocketServerConfig>,
    msg_queue: Arc<Mutex<VecDeque<WebSocketMessage>>>,
) {
    let thread_pool = AsyncComputeTaskPool::get();

    for request in server.filter_map(Result::ok) {
        let config = config.clone();
        let msg_queue = msg_queue.clone();
        thread_pool
            .spawn(handle_request(request, config, msg_queue))
            .detach();
    }
}

async fn handle_request(
    request: WsUpgrade<TcpStream, Option<Buffer>>,
    config: Arc<WebSocketServerConfig>,
    msg_queue: Arc<Mutex<VecDeque<WebSocketMessage>>>,
) {
    handle_request_inner(request, config, msg_queue)
        .await
        .unwrap_or_else(|error| error!("WebSocket request handler execution failed - {}", error))
}

async fn handle_request_inner(
    request: WsUpgrade<TcpStream, Option<Buffer>>,
    config: Arc<WebSocketServerConfig>,
    msg_queue: Arc<Mutex<VecDeque<WebSocketMessage>>>,
) -> Result<(), WebSocketError> {
    if !request.protocols().contains(&config.protocol) {
        request.reject().map_err(|(_, e)| e)?;
        return Ok(());
    }

    let mut client = request
        .use_protocol(&config.protocol)
        .accept()
        .map_err(|(_, e)| e)?;

    info!("New connection from: {}", client.peer_addr()?);

    for msg in client.incoming_messages() {
        let msg = msg?;

        if let OwnedMessage::Text(data) = msg {
            msg_queue.lock().push_back(WebSocketMessage { data });
        }
    }

    Ok(())
}
