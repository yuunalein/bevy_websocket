use std::net::TcpStream;

use bevy::prelude::*;
use tungstenite::Error;
use tungstenite::Utf8Bytes;
use tungstenite::{protocol::frame::Frame, Bytes};
use tungstenite::{Message, WebSocket};

/// Write data to a conversation.
#[derive(Resource)]
pub struct WebSocketWriter<'s> {
    pub(crate) stream: &'s mut WebSocket<TcpStream>,
}
impl WebSocketWriter<'_> {
    /// Send a message to the conversation.
    pub fn send_message(&mut self, data: impl Into<Utf8Bytes>) -> Result<(), Error> {
        self.stream.send(Message::Text(data.into()))
    }

    /// Send a binary to the conversation.
    pub fn send_binary(&mut self, data: impl Into<Bytes>) -> Result<(), Error> {
        self.stream.send(Message::Binary(data.into()))
    }

    /// Send a ping to the conversation.
    pub fn send_ping(&mut self, data: impl Into<Bytes>) -> Result<(), Error> {
        self.stream.send(Message::Ping(data.into()))
    }

    /// Send a raw [`Frame`] to the conversation.
    pub fn send_raw(&mut self, data: Frame) -> Result<(), Error> {
        self.stream.send(Message::Frame(data))
    }
}
