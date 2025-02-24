use std::collections::VecDeque;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use std::{
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
};

use bevy::log::LogPlugin;
use bevy::prelude::*;
use parking_lot::Mutex;
use tungstenite::accept_hdr;
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::http::{HeaderMap, HeaderValue, StatusCode};
use tungstenite::stream::MaybeTlsStream;

use crate::client::{Client, WebSocketClientMode, WebSocketClients};
use crate::peer::WebSocketPeer;
use crate::{events::*, WebSocketPlugin};

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

type RequestQueueInner = Arc<Mutex<VecDeque<MaybeTlsStream<TcpStream>>>>;

#[derive(Resource, Default, Deref)]
struct RequestQueue(RequestQueueInner);

pub(crate) fn install_websocket_server(app: &mut App, config: WebSocketServerConfig) -> &mut App {
    if !app.is_plugin_added::<WebSocketPlugin>() {
        const ERROR: &str = "WebSocketPlugin is required for WebSocketServerPlugin";

        if app.is_plugin_added::<LogPlugin>() {
            error!("{ERROR}");
            return app;
        } else {
            panic!("{ERROR}");
        }
    }

    let queue = RequestQueue::default();

    {
        let queue = queue.clone();
        let config = config.clone();

        thread::spawn(move || listen(config, queue));
    }

    app.insert_resource(config)
        .insert_resource(queue)
        .add_systems(Update, handle_request)
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
            Ok(req) => queue.lock_arc().push_back(MaybeTlsStream::Plain(req)),
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
            let peer = WebSocketPeer::from_maybe_tls_stream(&request)?;
            let mut mode: MaybeUninit<WebSocketClientMode> = MaybeUninit::uninit();
            let mut headers: MaybeUninit<HeaderMap<HeaderValue>> = MaybeUninit::uninit();

            if let Ok(stream) = accept_hdr(request, |request: &Request, response: Response| {
                handle_accept(request, response, &config, &mut mode, &mut headers)
            }) {
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
