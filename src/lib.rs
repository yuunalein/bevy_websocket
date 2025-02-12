#![warn(clippy::unwrap_used)]

pub mod events;
mod server;

use bevy::prelude::*;
use events::WebSocketMessage;
use server::*;

pub use server::WebSocketServerConfig;

pub struct WebSocketPlugin;
impl Plugin for WebSocketPlugin {
    fn build(&self, app: &mut App) {
        build(app.init_resource::<WebSocketServer>());
    }
}

pub struct CustomWebSocketPlugin(WebSocketServerConfig);
impl CustomWebSocketPlugin {
    pub fn new(config: WebSocketServerConfig) -> Self {
        Self(config)
    }
}
impl Plugin for CustomWebSocketPlugin {
    fn build(&self, app: &mut App) {
        build(app.insert_resource(WebSocketServer::new(self.0.clone())));
    }
}

fn build(app: &mut App) {
    app.add_event::<WebSocketMessage>()
        .add_systems(Startup, run_ws_server)
        .add_systems(Update, push_events);
}

fn run_ws_server(mut server: ResMut<WebSocketServer>) {
    server
        .run()
        .unwrap_or_else(|error| error!("Failed to start server - {error}"));
}

fn push_events(server: Res<WebSocketServer>, mut msg_event: EventWriter<WebSocketMessage>) {
    if !server.msg_queue.is_locked() {
        if let Some(message) = server.msg_queue.lock().pop_front() {
            msg_event.send(message);
        }
    }
}
