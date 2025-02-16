use std::net::TcpStream;

use bevy::prelude::*;
use websocket::{sync::Writer, OwnedMessage, WebSocketError};

#[derive(Resource)]
pub struct WebSocketWriter<'s> {
    pub(crate) sender: &'s mut Writer<TcpStream>,
}
impl WebSocketWriter<'_> {
    pub fn send_message<M: ToString>(&mut self, message: M) -> Result<(), WebSocketError> {
        self.sender
            .send_message(&OwnedMessage::Text(message.to_string()))
    }
}
