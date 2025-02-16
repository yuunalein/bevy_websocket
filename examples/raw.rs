use bevy::{log::LogPlugin, prelude::*};
use bevy_websocket::prelude::*;

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, LogPlugin::default(), WebSocketPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, on_message)
        .run();
}

fn setup() {
    println!(
        "Connect to this server with any tool, send a message and prepare for a silly response!"
    );
}

fn on_message(mut event: EventReader<WebSocketRawEvent>, mut clients: ResMut<WebSocketClients>) {
    for event in event.read() {
        if event.data.opcode == Opcode::Text {
            event
                .reply(&mut clients)
                .unwrap()
                .send_raw(&DataFrame::new(
                    true,
                    Opcode::Text,
                    "rawr 🐯".as_bytes().to_vec(),
                ))
                .unwrap();
        }
    }
}
