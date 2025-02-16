use std::net::SocketAddr;

use bevy::prelude::*;
pub use websocket::CloseData;

#[derive(Event, Debug)]
pub struct WebSocketMessage {
    pub data: String,
    pub peer: SocketAddr,
}

#[derive(Event, Debug)]
pub struct WebSocketBinary {
    pub data: Vec<u8>,
    pub peer: SocketAddr,
}

#[derive(Event, Debug)]
pub struct WebSocketOpen {
    pub peer: SocketAddr,
}

#[derive(Event, Debug)]
pub struct WebSocketClose {
    pub data: Option<CloseData>,
    pub peer: SocketAddr,
}
