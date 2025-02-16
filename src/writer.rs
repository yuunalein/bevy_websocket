use std::net::TcpStream;

use bevy::prelude::*;
pub use websocket::WebSocketError;
use websocket::{sync::Writer, ws::dataframe::DataFrame, OwnedMessage};

/// Write data to a conversation.
#[derive(Resource)]
pub struct WebSocketWriter<'s> {
    pub(crate) sender: &'s mut Writer<TcpStream>,
}
impl WebSocketWriter<'_> {
    /// Send a message to the conversation.
    pub fn send_message<M: ToString>(&mut self, data: M) -> Result<(), WebSocketError> {
        self.sender
            .send_message(&OwnedMessage::Text(data.to_string()))
    }

    /// Send a binary to the conversation.
    pub fn send_binary(&mut self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.sender.send_message(&OwnedMessage::Binary(data))
    }

    /// Send a ping to the conversation.
    pub fn send_ping(&mut self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.sender.send_message(&OwnedMessage::Ping(data))
    }

    /// Send a raw [`DataFrame`] to the conversation.
    pub fn send_raw<D: DataFrame>(&mut self, data: &D) -> Result<(), WebSocketError> {
        self.sender.send_dataframe(data)
    }
}
