use bevy::{log::LogPlugin, prelude::*};
use bevy_websocket::{
    prelude::*,
    tungstenite::protocol::frame::{
        coding::{Data, OpCode},
        Frame,
    },
};

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
        if event.data.header().opcode == OpCode::Data(Data::Text) {
            event
                .reply(&mut clients)
                .unwrap()
                .send_raw(Frame::message("rawr 🐯", OpCode::Data(Data::Text), true))
                .unwrap();
        }
    }
}
