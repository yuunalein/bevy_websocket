use std::{
    io::{stdin, stdout, Write},
    str::FromStr,
};

use bevy::{log::LogPlugin, prelude::*};
use bevy_websocket::{
    prelude::*,
    tungstenite::{self, client::ClientRequestBuilder, http::Uri},
};

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, LogPlugin::default(), WebSocketPlugin))
        .add_systems(Startup, setup)
        .add_systems(Update, on_message)
        .run();
}

const DEFAULT_PROTOCOL: &str = "bevy_websocket";

fn setup(mut clients: ResMut<WebSocketClients>) {
    println!(
        "This is a readonly client implementation. (once the connection is established you can\
read all incoming messages but you're not able to send any.)"
    );
    loop {
        match build_request(&mut clients) {
            Ok(_) => break,
            Err(e) => error!("{e}"),
        }
    }
}

fn build_request(clients: &mut ResMut<WebSocketClients>) -> Result<(), tungstenite::Error> {
    let uri = {
        print!("uri (ws://, wss://): ");
        stdout().flush()?;

        let mut buf = String::new();
        stdin().read_line(&mut buf)?;

        buf
    };

    let protocol = {
        print!("protocol (default: {DEFAULT_PROTOCOL}): ");
        stdout().flush()?;

        let mut buf = String::new();
        stdin().read_line(&mut buf)?;

        if buf.trim() == "" {
            DEFAULT_PROTOCOL.to_owned()
        } else {
            buf
        }
    };

    let request =
        ClientRequestBuilder::new(Uri::from_str(uri.trim())?).with_sub_protocol(protocol.trim());
    clients.request(request, WebSocketClientMode::Parsed)?;

    Ok(())
}

fn on_message(mut events: EventReader<WebSocketMessageEvent>) {
    for message in events.read() {
        println!("{}: {}", message.peer, message.data);
    }
}
