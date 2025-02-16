# Bevy WebSocket

[![License](https://img.shields.io/github/license/yuunalein/bevy_websocket)](https://raw.githubusercontent.com/yuunalein/bevy_websocket/refs/heads/main/LICENSE)

A WebSocket server that runs in bevy.

## Usage

1. Add `bevy_websocket` to your `Cargo.toml`
2. Add `bevy_websocket::WebSocketPlugin` to your bevy `App`
3. Receive messages with `EventReader<WebSocketMessageEvent>` or send any with `ResMut<WebSocketClients>`

---

bevy_websocket is currently not available on crates.io. [see why.](https://github.com/yuunalein/bevy_websocket/issues/1)

Add bevy_websocket with following line to your Cargo dependencies.

```toml
bevy_websocket = { git = "https://github.com/yuunalein/bevy_websocket.git", tag = "v0.2.0" }
```

---

### Examples

Run the `messenger` example to see the crate in action.

```shell
cargo run --example messenger
```

Or implement this code in your project.

```rust
use bevy::prelude::*;
use bevy_websocket::prelude::*;

fn main() {
    App::new()
        .add_plugins((MinimalPlugins, WebSocketPlugin))
        .add_systems(Update, on_message)
        .run();
}

fn on_message(
    mut event: EventReader<WebSocketMessageEvent>,
    mut clients: ResMut<WebSocketClients>,
) {
    for message in event.read() {
        println!("Received {}", message.data);

        if message.data == "ping" {
            message
                .reply(&mut clients)
                .unwrap()
                .send_message("Pong!")
                .unwrap();
        }
    }
}
```

## Bevy Version Support

| bevy | bevy_websocket |
| ---- | -------------- |
| 0.15 | 0.1, 0.2       |

---

This crate was made with ❤️
