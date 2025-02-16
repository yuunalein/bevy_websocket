use bevy::prelude::*;
pub use websocket::{
    dataframe::{DataFrame, Opcode},
    CloseData,
};

use crate::{
    server::{WebSocketClients, WebSocketPeer},
    writer::WebSocketWriter,
    WebSocketClientMode,
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

            pub fn set_mode(
                &self,
                clients: &mut WebSocketClients,
                mode: WebSocketClientMode,
            ) -> Option<()> {
                self.peer.set_mode(clients, mode)
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
    WebSocketOpenEvent,
    WebSocketRawEvent
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
pub struct WebSocketRawEvent {
    pub data: DataFrame,
    pub peer: WebSocketPeer,
}

#[derive(Event, Debug)]
pub struct WebSocketOpenEvent {
    pub peer: WebSocketPeer,
    pub mode: WebSocketClientMode,
}

#[derive(Event, Debug)]
pub struct WebSocketCloseEvent {
    pub data: Option<CloseData>,
    pub peer: WebSocketPeer,
}
