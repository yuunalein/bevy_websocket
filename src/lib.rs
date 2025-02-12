#![warn(clippy::unwrap_used)]

pub mod events;
mod server;

use bevy::prelude::*;
use events::*;
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
        .add_event::<WebSocketBinary>()
        .add_event::<WebSocketOpen>()
        .add_event::<WebSocketClose>()
        .add_systems(Startup, run_ws_server)
        .add_systems(Update, push_events);
}

fn run_ws_server(mut server: ResMut<WebSocketServer>) {
    server
        .run()
        .unwrap_or_else(|error| error!("Failed to start server - {error}"));
}

fn push_events(
    server: Res<WebSocketServer>,
    mut message_w: EventWriter<WebSocketMessage>,
    mut binary_w: EventWriter<WebSocketBinary>,
    mut open_w: EventWriter<WebSocketOpen>,
    mut close_w: EventWriter<WebSocketClose>,
) {
    if !server.queue.is_locked() {
        if let Some(event) = server.queue.lock_arc().pop_front() {
            match event {
                events::EventKind::Message(message) => {
                    message_w.send(message);
                }
                events::EventKind::Binary(binary) => {
                    binary_w.send(binary);
                }
                events::EventKind::Open(open) => {
                    open_w.send(open);
                }
                events::EventKind::Close(close) => {
                    close_w.send(close);
                }
            }
        }
    }
}
