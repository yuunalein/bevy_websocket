use std::{env::current_dir, net::SocketAddr};

use bevy::prelude::*;
use bevy_websocket::prelude::*;

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, WebSocketPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, (on_connect, on_auth, on_message, on_disconnect))
        .run();
}

fn setup() {
    if let Ok(path) = current_dir() {
        println!(
            "Open file://{}/examples/messenger.html to start messaging.",
            path.display()
        );
    } else {
        println!("Open messenger.html to start messaging.")
    }
}

#[derive(Debug, Component)]
struct ClientName {
    name: String,
}

#[derive(Debug, Component)]
struct Client {
    peer: SocketAddr,
}

fn on_connect(
    mut commands: Commands,
    mut event: EventReader<WebSocketOpen>,
    mut writer: ResMut<WebSocketWriter>,
) {
    for open in event.read() {
        commands.spawn(Client { peer: open.peer });

        // This "handshake" is required since the other systems
        // require Client to exist.
        if writer.send_message("$$hello$$", &open.peer).is_err() {
            println!("Failed to deliver hello to {}", open.peer);
        }

        println!("New connection from: {}", open.peer);
    }
}

fn on_auth(
    mut commands: Commands,
    mut event: EventReader<WebSocketMessage>,
    query: Query<(Entity, &Client)>,
) {
    for message in event.read() {
        if let Some(name) = message.data.strip_prefix("$$auth$$") {
            for (entity, client) in query.iter() {
                if client.peer == message.peer {
                    commands.entity(entity).insert(ClientName {
                        name: name.to_string(),
                    });
                    println!("{} identified as: {}", client.peer, name);
                    break;
                }
            }
        }
    }
}

fn on_message(
    mut event: EventReader<WebSocketMessage>,
    query: Query<(&ClientName, &Client)>,
    mut writer: ResMut<WebSocketWriter>,
) {
    for message in event.read() {
        for (name, client) in query.iter() {
            if client.peer == message.peer {
                for (_, client) in query.iter() {
                    if writer
                        .send_message(format!("{}: {}", name.name, message.data), &client.peer)
                        .is_err()
                    {
                        println!("Failed to deliver message to {}", client.peer);
                    }
                }
                println!("{}: {}", name.name, message.data);
                break;
            }
        }
    }
}

fn on_disconnect(
    mut commands: Commands,
    mut event: EventReader<WebSocketClose>,
    query: Query<(Entity, &Client, &ClientName)>,
) {
    for close in event.read() {
        for (entity, client, name) in query.iter() {
            if client.peer == close.peer {
                println!("{} disconnected.", name.name);
                commands.entity(entity).despawn();
                break;
            }
        }
    }
}
