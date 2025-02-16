use std::net::SocketAddr;

use bevy::prelude::*;
pub use websocket::CloseData;

#[derive(Event, Debug)]
pub struct WebSocketMessageEvent {
    pub data: String,
    pub peer: SocketAddr,
}

#[derive(Event, Debug)]
pub struct WebSocketBinaryEvent {
    pub data: Vec<u8>,
    pub peer: SocketAddr,
}

#[derive(Event, Debug)]
pub struct WebSocketPongEvent {
    pub data: Vec<u8>,
    pub peer: SocketAddr,
}

#[derive(Event, Debug)]
pub struct WebSocketOpenEvent {
    pub peer: SocketAddr,
}

#[derive(Event, Debug)]
pub struct WebSocketCloseEvent {
    pub data: Option<CloseData>,
    pub peer: SocketAddr,
}
