#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

mod camera;
mod chaser;
mod config;
mod cursor;
mod debug;
mod debug_overlay;
mod flowfield;
// mod framerate;
mod gamepad;
mod physics;
mod player;
mod point;
mod segment;
mod spawner;
mod tilemap;

use bevy::core::FrameCount;
use bevy::input::common_conditions::input_toggle_active;
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowMode, WindowResolution, WindowTheme};
use bevy_fast_tilemap::prelude::*;
use bevy_framepace::{FramepaceSettings, Limiter};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_prototype_lyon::plugin::ShapePlugin;
use bevy_svg::SvgPlugin;
use camera::CameraPlugin;
use chaser::ChaserPlugin;
use config::{Config, ConfigPlugin};
use cursor::CursorPlugin;
use debug::DebugPlugin;
use debug_overlay::DebugOverlayPlugin;
use flowfield::FlowfieldPlugin;
use gamepad::GamepadPlugin;
use physics::PhysicsPlugin;
use player::PlayerPlugin;
use spawner::SpawnPlugin;
use tilemap::MyTilemapPlugin;

const BACKGROUND_COLOR: Color = Color::srgb(0.05, 0.065, 0.08);
const WINDOW_WIDTH: f32 = 3440.;
const WINDOW_HEIGHT: f32 = 1361.;

fn main() {
    let window_resolution =
        WindowResolution::new(WINDOW_WIDTH, WINDOW_HEIGHT).with_scale_factor_override(1.0);

    App::new()
        // Bevy built-ins
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "train game".into(),
                        name: Some("train_game.app".into()),
                        resolution: window_resolution,
                        mode: WindowMode::Windowed,
                        fit_canvas_to_parent: true,
                        present_mode: PresentMode::Immediate,
                        prevent_default_event_handling: false,
                        window_theme: Some(WindowTheme::Dark),
                        visible: false,
                        ..default()
                    }),
                    ..default()
                })
                .set(ImagePlugin::default_linear()),
        )
        .add_plugins(bevy_framepace::FramepacePlugin)
        .add_plugins(FastTileMapPlugin::default())
        .add_plugins(ShapePlugin)
        .add_plugins(SvgPlugin)
        // Bevy inspector egui
        .add_plugins(WorldInspectorPlugin::new().run_if(input_toggle_active(false, KeyCode::KeyX)))
        // User plugins
        .add_plugins(ConfigPlugin)
        .add_plugins(CursorPlugin)
        .add_plugins(CameraPlugin)
        .add_plugins(FlowfieldPlugin)
        .add_plugins(GamepadPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(SpawnPlugin)
        .add_plugins(MyTilemapPlugin)
        .add_plugins(DebugPlugin)
        // .add_plugins(FrametimePlugin)
        .add_plugins(DebugOverlayPlugin)
        .add_plugins(PlayerPlugin)
        .add_plugins(ChaserPlugin)
        .add_systems(Startup, setup_window)
        .add_systems(
            Update,
            (
                make_visible,
                update_framerate_target,
                toggle_vsync,
                exit_app,
            ),
        )
        .insert_resource(ClearColor(BACKGROUND_COLOR))
        .run();
}

fn exit_app(keyboard: Res<ButtonInput<KeyCode>>, mut exit: EventWriter<AppExit>) {
    if keyboard.just_pressed(KeyCode::Escape) {
        println!("Exiting app");
        exit.send(AppExit::Success);
    }
}

fn setup_window(mut q_window: Query<&mut Window>, mut settings: ResMut<FramepaceSettings>) {
    // Turn off frame limiter by default
    settings.limiter = Limiter::Off;

    // maximize window
    q_window.single_mut().set_maximized(true);
}

/// Set framerate target through config.
fn update_framerate_target(
    config: Res<Config>,
    mut settings: ResMut<FramepaceSettings>,
    mut framerate: Local<f64>,
) {
    /// Framerate target cannot be below this value
    const FRAMERATE_MIN: f64 = 10.0;

    // 0.0 means uncapped framerate
    let new_framerate = if config.framerate < FRAMERATE_MIN {
        0.0
    } else {
        config.framerate
    };

    // Don't update if new framerate is the same as old
    if new_framerate == *framerate {
        return;
    }
    *framerate = new_framerate;

    // Update target
    if new_framerate == 0.0 {
        settings.limiter = Limiter::Off;
    } else {
        settings.limiter = Limiter::from_framerate(new_framerate);
    }
}

/// Wait until things are ready before showing window.
fn make_visible(framecount: Res<FrameCount>, mut q_window: Query<&mut Window>) {
    if framecount.0 == 4 {
        q_window.single_mut().visible = true;
    }
}

/// Toggles vsync when "V" key pressed.
fn toggle_vsync(input: Res<ButtonInput<KeyCode>>, mut windows: Query<&mut Window>) {
    if !input.just_pressed(KeyCode::KeyV) {
        return;
    }
    let mut window = windows.single_mut();

    window.present_mode = if matches!(window.present_mode, PresentMode::AutoVsync) {
        PresentMode::AutoNoVsync
    } else {
        PresentMode::AutoVsync
    };
    info!("PRESENT_MODE: {:?}", window.present_mode);
}
