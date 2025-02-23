use bevy::prelude::*;
use tungstenite::{
    http::{HeaderMap, HeaderValue},
    protocol::{frame::Frame, CloseFrame},
    Bytes,
};

use crate::{
    client::{WebSocketClientMode, WebSocketClients},
    peer::WebSocketPeer,
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

/// This event represents text messages.
#[derive(Event, Debug)]
pub struct WebSocketMessageEvent {
    pub data: String,
    pub peer: WebSocketPeer,
}

/// This event represents binary data.
#[derive(Event, Debug)]
pub struct WebSocketBinaryEvent {
    pub data: Bytes,
    pub peer: WebSocketPeer,
}

/// This event represents ping replies (pong).
#[derive(Event, Debug)]
pub struct WebSocketPongEvent {
    pub data: Bytes,
    pub peer: WebSocketPeer,
}

/// This event represents raw frames.
#[derive(Event, Debug)]
pub struct WebSocketRawEvent {
    pub data: Frame,
    pub peer: WebSocketPeer,
}

/// This event represents that a new conversation has been established.
#[derive(Event, Debug)]
pub struct WebSocketOpenEvent {
    pub peer: WebSocketPeer,
    pub mode: WebSocketClientMode,
    pub headers: HeaderMap<HeaderValue>,
}

/// This event represents that a conversation has been closed.
#[derive(Event, Debug)]
pub struct WebSocketCloseEvent {
    pub data: Option<CloseFrame>,
    pub peer: WebSocketPeer,
}
