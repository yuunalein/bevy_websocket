#![warn(clippy::unwrap_used)]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod events;
pub mod peer;
pub mod server;
pub mod writer;

pub mod prelude {
    pub use crate::client::*;
    pub use crate::events::*;
    pub use crate::peer::*;
    pub use crate::server::*;
    pub use crate::writer::*;
    pub use crate::WebSocketPlugin;
    pub use crate::WebSocketServerPlugin;
}

use bevy::prelude::*;
use client::*;
use events::*;
use server::*;

pub use tungstenite;

/// This plugin will add support for WebSocket communication to a Bevy Application.
pub struct WebSocketPlugin;
impl Plugin for WebSocketPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WebSocketClients>()
            .add_event::<WebSocketMessageEvent>()
            .add_event::<WebSocketBinaryEvent>()
            .add_event::<WebSocketPongEvent>()
            .add_event::<WebSocketRawEvent>()
            .add_event::<WebSocketOpenEvent>()
            .add_event::<WebSocketCloseEvent>()
            .add_systems(Update, handle_clients);
    }
}

/// This plugin will run a WebSocket server in a Bevy Application.
pub struct WebSocketServerPlugin;
impl Plugin for WebSocketServerPlugin {
    fn build(&self, app: &mut App) {
        install_websocket_server(app, WebSocketServerConfig::default());
    }
}
impl WebSocketServerPlugin {
    /// Customize the plugin with a [`WebSocketServerConfig`]
    pub fn custom(config: WebSocketServerConfig) -> CustomWebSocketServerPlugin {
        CustomWebSocketServerPlugin(config)
    }
}

pub struct CustomWebSocketServerPlugin(WebSocketServerConfig);
impl Plugin for CustomWebSocketServerPlugin {
    fn build(&self, app: &mut App) {
        install_websocket_server(app, self.0.clone());
    }
}
