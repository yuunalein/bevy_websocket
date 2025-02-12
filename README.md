# Bevy WebSocket

[![License](https://img.shields.io/github/license/yuunalein/bevy_websocket)](https://raw.githubusercontent.com/yuunalein/bevy_websocket/refs/heads/main/LICENSE)

A WebSocket server that runs in bevy.

## Usage

1. Add `bevy_websocket` to your `Cargo.toml`
2. Add `bevy_websocket::WebSocketPlugin` to your bevy `App`
3. Receive messages with `EventReader<WebSocketMessage>` or send any with `ResMut<WebSocketWriter>`

### Example

```rust
fn main() {
    App::new()
        .add_plugins(WebSocketPlugin)
        .add_systems(Update, on_message)
        .run();
}

fn on_message(
    mut event: EventReader<WebSocketMessage>,
    mut writer: ResMut<WebSocketWriter>,
) {
    for message in event.read() {
        println!("Received {}", message.data);

        if message.data == "ping" {
            writer.send_message("Pong!", &message.peer).unwrap();
        }
    }
}
```

---

This crate was made with ❤️
