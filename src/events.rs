use bevy::prelude::*;
pub use websocket::CloseData;

use crate::{
    server::{WebSocketClients, WebSocketPeer},
    writer::WebSocketWriter,
};

macro_rules! impl_reply {
    ($t:ty) => {
        impl $t {
            pub fn reply<'c>(
                &self,
                clients: &'c mut WebSocketClients,
            ) -> Option<WebSocketWriter<'c>> {
                self.peer.write(clients)
            }
        }
    };

    ($first:ty $(, $rest:ty)*) => {
        impl_reply!($first);
        impl_reply!($($rest),*);
    }
}

impl_reply!(
    WebSocketMessageEvent,
    WebSocketBinaryEvent,
    WebSocketPongEvent,
    WebSocketOpenEvent
);

#[derive(Event, Debug)]
pub struct WebSocketMessageEvent {
    pub data: String,
    pub peer: WebSocketPeer,
}

#[derive(Event, Debug)]
pub struct WebSocketBinaryEvent {
    pub data: Vec<u8>,
    pub peer: WebSocketPeer,
}

#[derive(Event, Debug)]
pub struct WebSocketPongEvent {
    pub data: Vec<u8>,
    pub peer: WebSocketPeer,
}

#[derive(Event, Debug)]
pub struct WebSocketOpenEvent {
    pub peer: WebSocketPeer,
}

#[derive(Event, Debug)]
pub struct WebSocketCloseEvent {
    pub data: Option<CloseData>,
    pub peer: WebSocketPeer,
}
