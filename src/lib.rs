#![warn(clippy::unwrap_used)]

pub mod events;
mod server;
pub mod writer;

pub mod prelude {
    pub use crate::events::*;
    pub use crate::server::*;
    pub use crate::writer::*;
    pub use crate::WebSocketPlugin;
}

use bevy::prelude::*;
use server::*;

pub use server::*;

pub struct WebSocketPlugin;
impl Plugin for WebSocketPlugin {
    fn build(&self, app: &mut App) {
        install_websocket_server(app, WebSocketServerConfig::default());
    }
}
impl WebSocketPlugin {
    pub fn custom(config: WebSocketServerConfig) -> CustomWebSocketPlugin {
        CustomWebSocketPlugin(config)
    }
}

pub struct CustomWebSocketPlugin(WebSocketServerConfig);
impl Plugin for CustomWebSocketPlugin {
    fn build(&self, app: &mut App) {
        install_websocket_server(app, self.0.clone());
    }
}
