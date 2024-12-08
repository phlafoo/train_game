use bevy::{
    input::gamepad::{GamepadConnection, GamepadEvent},
    prelude::*,
};

/// Handles connection to gamepad
pub struct GamepadPlugin;

impl Plugin for GamepadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MyGamepads>()
            .add_systems(Update, gamepad_connections);
    }
}

/// Stores the ID of the first connected gamepad.
#[derive(Resource, Default)]
pub struct MyGamepads(pub Vec<Gamepad>);

fn gamepad_connections(
    mut my_gamepads: ResMut<MyGamepads>,
    mut evr_gamepad: EventReader<GamepadEvent>,
) {
    for ev in evr_gamepad.read() {
        // only interested in connection events
        let GamepadEvent::Connection(ev_conn) = ev else {
            continue;
        };
        match &ev_conn.connection {
            GamepadConnection::Connected(info) => {
                debug!(
                    "New gamepad connected: {:?}, name: {}",
                    ev_conn.gamepad, info.name,
                );
                // Add this gamepad to our gamepads resource
                my_gamepads.0.push(ev_conn.gamepad);
            }
            GamepadConnection::Disconnected => {
                debug!("Lost connection with gamepad: {:?}", ev_conn.gamepad);
                let Some(index) = my_gamepads.0.iter().position(|g| *g == ev_conn.gamepad) else {
                    continue;
                };
                // Remove gamepad from our gamepads resource
                my_gamepads.0.remove(index);
            }
        }
    }
}
