use std::net::TcpStream;

use bevy::prelude::*;
pub use websocket::WebSocketError;
use websocket::{sync::Writer, OwnedMessage};

#[derive(Resource)]
pub struct WebSocketWriter<'s> {
    pub(crate) sender: &'s mut Writer<TcpStream>,
}
impl WebSocketWriter<'_> {
    pub fn send_message<M: ToString>(&mut self, data: M) -> Result<(), WebSocketError> {
        self.sender
            .send_message(&OwnedMessage::Text(data.to_string()))
    }

    pub fn send_binary(&mut self, data: Vec<u8>) -> Result<(), WebSocketError> {
        self.sender.send_message(&OwnedMessage::Binary(data))
    }
}
