use bevy::prelude::*;

#[derive(Event, Debug)]
pub struct WebSocketMessage {
    pub data: String,
}
