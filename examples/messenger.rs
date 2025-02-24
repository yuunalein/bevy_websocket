use std::{
    env::current_dir,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
};

use bevy::prelude::*;
use bevy_websocket::prelude::*;

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            WebSocketPlugin,
            WebSocketServerPlugin::custom(WebSocketServerConfig {
                addr: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 42069)),
                ..default()
            }),
        ))
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
    peer: WebSocketPeer,
}

fn on_connect(
    mut commands: Commands,
    mut event: EventReader<WebSocketOpenEvent>,
    mut clients: ResMut<WebSocketClients>,
) {
    for open in event.read() {
        commands.spawn(Client { peer: open.peer });

        // This "handshake" is required since the other systems
        // require Client to exist.
        if let Some(mut writer) = open.reply(&mut clients) {
            if writer.send_message("$$hello$$").is_err() {
                println!("Failed to deliver hello to {}", open.peer);
            }
        } else {
            println!("{} has closed already.", open.peer);
        }

        println!("New connection from: {}", open.peer);
    }
}

fn on_auth(
    mut commands: Commands,
    mut event: EventReader<WebSocketMessageEvent>,
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
    mut event: EventReader<WebSocketMessageEvent>,
    query: Query<(&ClientName, &Client)>,
    mut clients: ResMut<WebSocketClients>,
) {
    for message in event.read() {
        for (name, client) in query.iter() {
            if client.peer == message.peer {
                for (_, client) in query.iter() {
                    if let Some(mut writer) = client.peer.write(&mut clients) {
                        if writer
                            .send_message(format!("{}: {}", name.name, message.data))
                            .is_err()
                        {
                            println!("Failed to deliver message to {}", client.peer);
                        }
                    } else {
                        println!("{} has closed already.", client.peer);
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
    mut event: EventReader<WebSocketCloseEvent>,
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
