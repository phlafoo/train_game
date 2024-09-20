use std::{f32::consts::PI, time::Duration};

use bevy::{
    color::palettes::css::*,
    core::FrameCount,
    math::vec2,
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
    time::Stopwatch,
};
use bevy_rapier2d::{dynamics::Velocity, prelude::*};
use bevy_svg::prelude::*;

use crate::{
    camera::{CameraRange, MainCamera},
    chaser::Chaser,
    config::Config,
    gamepad::MyGamepad,
    physics::PLAYER_GROUP,
    spawner::Spawner,
    tilemap::{Args, PlayerSpawn},
};

const AVERAGE_SPEED_INTERVAL: f32 = 0.08;
const COLOR_TRANSITION_TIME: f32 = 0.4;
const PLAYER_RADIUS: f32 = 8.0;

const DAMPING: f32 = 0.0;
const PLAYER_COLOR: Srgba = Srgba::rgb(3.0 / 255.0, 221.0 / 255.0, 1.0);
const COLLISION_COLOR: Srgba = RED;

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostStartup,
            spawn_player.after(TransformSystem::TransformPropagate),
        )
        .add_systems(Startup, spawn_velocity_text)
        .add_systems(
            PostUpdate,
            (benchmark, player_movement)
                .chain()
                .before(RapierTransformPropagateSet),
        )
        .add_systems(
            Update,
            (
                reset_player,
                toggle_noclip,
                handle_collision_event,
                transition_color,
                update_velocity_ui,
            ),
        )
        .register_type::<Player>();
    }
}

#[derive(Component, Debug, Reflect)]
pub struct Player {
    /// Also affects cornering.
    pub acceleration: f32,
    pub boost_acceleration: f32,
    /// No input deceleration.
    pub brake_acceleration: f32,
    pub max_speed: f32,
    pub boost_max_speed: f32,
    /// How quickly speed reduces to max speed.
    pub drag: f32,
    /// Very fast. For debug purposes.
    pub debug_max_speed: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            acceleration: 40.0,
            boost_acceleration: 24.0,
            brake_acceleration: 20.0,
            max_speed: 320.0,
            boost_max_speed: 480.0,
            drag: 0.1,
            debug_max_speed: 960.0,
        }
    }
}

/// Used for color change on collision
#[derive(Component, Debug)]
pub struct TimeSinceCollision {
    time: Stopwatch,
}

/// Draws average speed of player on screen
#[derive(Component)]
pub struct SpeedUi;

#[derive(Component, Debug, Default)]
struct AverageSpeed {
    speed_sum: f32,
    samples: u32,
    last_reset_secs: f32,
}

pub fn spawn_player(
    q_spawn: Query<&GlobalTransform, With<PlayerSpawn>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut q_camera: Query<(&mut Transform, &CameraRange), With<MainCamera>>,
) {
    let spawn_pos = q_spawn.single();
    let spawn_pos = spawn_pos.translation().with_z(5.0);
    info!("{:?}", spawn_pos);

    let mut stopwatch = Stopwatch::new();
    stopwatch.set_elapsed(Duration::from_secs_f32(COLOR_TRANSITION_TIME));

    let svg = asset_server.load("svgs/player.svg");

    commands
        .spawn((
            Player::default(),
            Name::new("Player"),
            Svg2dBundle {
                svg,
                origin: Origin::TopLeft,
                transform: Transform {
                    translation: spawn_pos,
                    scale: Vec3::ONE,
                    ..default()
                },
                ..default()
            },
            ExternalForce::default(),
            Damping {
                linear_damping: DAMPING,
                ..default()
            },
            Velocity::default(),
            RigidBody::Dynamic,
            Ccd::enabled(),
            // SoftCcd { prediction: 8.0 },
            Collider::ball(PLAYER_RADIUS - 1.0),
            CollisionGroups::new(PLAYER_GROUP, Group::ALL),
            ActiveEvents::CONTACT_FORCE_EVENTS | ActiveEvents::COLLISION_EVENTS,
            Friction {
                coefficient: 0.0,
                combine_rule: CoefficientCombineRule::Min,
            },
            Restitution {
                coefficient: 0.0,
                combine_rule: CoefficientCombineRule::Average,
            },
            AverageSpeed::default(),
            ReadMassProperties::default(),
        ))
        .with_children(|parent| {
            // SVG cannot be tinted easily so we spawn this child that can change color easily
            parent.spawn((
                MaterialMesh2dBundle {
                    mesh: Mesh2dHandle(meshes.add(Circle::new(PLAYER_RADIUS - 1.0))),
                    material: materials.add(ColorMaterial::from_color(PLAYER_COLOR)),
                    transform: Transform::from_xyz(0.0, 0.0, -1.0),
                    ..default()
                },
                TimeSinceCollision { time: stopwatch },
            ));
        });

    let (mut cam_transform, cam_range) = q_camera.single_mut();
    cam_transform.translation = spawn_pos.clamp(cam_range.min, cam_range.max);
}

fn spawn_velocity_text(mut commands: Commands) {
    commands.spawn((
        SpeedUi,
        TextBundle::from_section(
            "",
            TextStyle {
                font_size: 30.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(5.0),
            left: Val::Px(5.0),
            ..default()
        }),
    ));
}

fn update_velocity_ui(
    mut text_query: Query<&mut Text, With<SpeedUi>>,
    mut player_query: Query<(&Velocity, &mut AverageSpeed), With<Player>>,
    time: Res<Time>,
) {
    let Ok((v, mut ave_speed)) = player_query.get_single_mut() else {
        return;
    };

    ave_speed.samples += 1;
    ave_speed.speed_sum += v.linvel.x;

    if (time.elapsed_seconds() - ave_speed.last_reset_secs) < AVERAGE_SPEED_INTERVAL {
        // Don't update text until next interval
        return;
    }
    // time to update text
    let average_speed = ave_speed.speed_sum / ave_speed.samples as f32;

    ave_speed.last_reset_secs = time.elapsed_seconds();
    ave_speed.speed_sum = 0.0;
    ave_speed.samples = 0;

    let mut text = text_query.single_mut();
    text.sections[0].value = format!("{0:>6.1}", average_speed);
}

/// Reset player position, despawn all chasers, and deactivate spawners.
fn reset_player(
    mut commands: Commands,
    mut player_query: Query<(&mut Transform, &mut ExternalForce, &mut Velocity), With<Player>>,
    chaser_query: Query<Entity, With<Chaser>>,
    k: Res<ButtonInput<KeyCode>>,
    q_player_spawn: Query<(&PlayerSpawn, &GlobalTransform)>,
    mut q_spawner: Query<&mut Spawner>,
) {
    if k.just_pressed(KeyCode::KeyR) {
        let (mut t, mut f, mut v) = player_query.single_mut();
        let (_, spawn_transform) = q_player_spawn.single();
        t.translation = spawn_transform.translation();
        f.force = Vec2::ZERO;
        v.linvel = Vec2::ZERO;

        for entity in chaser_query.iter() {
            commands.entity(entity).despawn_recursive();
        }
        for mut spawner in q_spawner.iter_mut() {
            spawner.timer = if spawner.immediate {
                Timer::new(Duration::ZERO, TimerMode::Repeating)
            } else {
                Timer::from_seconds(spawner.delay, TimerMode::Repeating)
            };
            spawner.count = 0;
            spawner.active = spawner.active_default;
        }
    }
}

/// Have consistent benchmarking for tracing
fn benchmark(
    args: Res<Args>,
    framecount: Res<FrameCount>,
    mut exit: EventWriter<AppExit>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
) {
    if !args.bench {
        return;
    }
    // Don't run for too long because the trace files become massive quickly
    if framecount.0 == 350 {
        exit.send(AppExit::Success);
        return;
    }
    // Move left fast
    keyboard.press(KeyCode::ShiftLeft);
    keyboard.press(KeyCode::ArrowLeft);
}

/// Let player pass through objects. Toggle with "N" key.
fn toggle_noclip(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    query_sensor: Query<Entity, (With<Player>, With<Sensor>)>,
    query: Query<Entity, (With<Player>, Without<Sensor>)>,
) {
    if !keyboard.just_pressed(KeyCode::KeyN) {
        return;
    }
    if let Ok(entity) = query.get_single() {
        commands.get_entity(entity).unwrap().insert(Sensor);
    } else {
        let entity = query_sensor.single();
        commands.get_entity(entity).unwrap().remove::<Sensor>();
    }
}

/// Input gets directly mapped to this.
#[derive(Clone, Copy, Debug)]
struct PlayerAction {
    /// Desired direction of movement.
    move_dir: Vec2,
    /// Go faster.
    boost: bool,
    /// Go much faster (for debug purposes).
    debug_boost: bool,
}

/// Get [`PlayerAction`] based on user input
fn get_player_action(
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad: Option<Res<MyGamepad>>,
    axes: Res<Axis<GamepadAxis>>,
    gamepad_buttons: Res<ButtonInput<GamepadButton>>,
    config: Res<Config>,
) -> PlayerAction {
    let mut boost = false;
    let mut debug_boost = false;

    let mut move_dir = Vec2::ZERO;

    if keyboard.get_pressed().next().is_some() {
        if keyboard.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
            move_dir += Vec2::Y;
        }
        if keyboard.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
            move_dir += Vec2::NEG_X;
        }
        if keyboard.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
            move_dir += Vec2::NEG_Y;
        }
        if keyboard.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
            move_dir += Vec2::X;
        }
        if keyboard.pressed(KeyCode::ShiftLeft) {
            boost = true;
        }
        if keyboard.pressed(KeyCode::AltLeft) {
            debug_boost = true;
        }
    }

    if let Some(&MyGamepad(gamepad)) = gamepad.as_deref() {
        let axis_lx = GamepadAxis {
            gamepad,
            axis_type: GamepadAxisType::LeftStickX,
        };
        let axis_ly = GamepadAxis {
            gamepad,
            axis_type: GamepadAxisType::LeftStickY,
        };
        if let (Some(mut x), Some(mut y)) = (axes.get(axis_lx), axes.get(axis_ly)) {
            if x.abs() < config.stick_deadzone {
                x = 0.0;
            }
            if y.abs() < config.stick_deadzone {
                y = 0.0;
            }

            move_dir += vec2(x, y);
        }
        if gamepad_buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::South,
        }) {
            boost = true;
        }
        if gamepad_buttons.pressed(GamepadButton {
            gamepad,
            button_type: GamepadButtonType::RightTrigger,
        }) {
            debug_boost = true;
        }
    }
    PlayerAction {
        move_dir,
        boost,
        debug_boost,
    }
}

// TODO fix framerate affecting accel
fn player_movement(
    config: Res<Config>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepad_buttons: Res<ButtonInput<GamepadButton>>,
    axes: Res<Axis<GamepadAxis>>,
    time: Res<Time>,
    gamepad: Option<Res<MyGamepad>>,
    mut query: Query<(&mut Velocity, &mut Transform, &Player)>,
) {
    let Ok((mut velocity, mut transform, player)) = query.get_single_mut() else {
        return;
    };

    let mut action = get_player_action(keyboard, gamepad, axes, gamepad_buttons, config);

    let v = &mut velocity.linvel;
    let mag_before = v.length();
    let dt = time.delta_seconds();

    // BRAKE
    if action.move_dir == Vec2::ZERO {
        let s = (player.brake_acceleration * dt).min(1.0);
        *v = v.lerp(Vec2::ZERO, s);
        return;
    }

    action.move_dir = action.move_dir.clamp_length_max(1.0);

    let (mut max_speed, accel) = if action.debug_boost {
        (player.debug_max_speed, player.boost_acceleration)
    } else if action.boost {
        (player.boost_max_speed, player.boost_acceleration)
    } else {
        (player.max_speed, player.acceleration)
    };

    let angle = action.move_dir.to_angle() - PI * 0.75;
    transform.rotation = Quat::from_rotation_z(angle);

    if mag_before >= max_speed {
        max_speed = mag_before.lerp(max_speed, player.drag);
    }
    let new_velocity = action.move_dir * max_speed;

    let s = (dt * accel).min(1.0);
    *v = v.lerp(new_velocity, s);
}

/// If player touches chaser we need to start the timer for color change.
fn handle_collision_event(
    mut contact_events: EventReader<ContactForceEvent>,
    mut q_stopwatch: Query<&mut TimeSinceCollision>,
    q_chaser: Query<&Chaser>,
) {
    for event in contact_events.read() {
        if q_chaser.get(event.collider2).is_ok() {
            // Player is in contact with a chaser! Reset timer to trigger color change
            q_stopwatch.single_mut().time.reset();

            // If multiple chasers are in contact, this ensures that those events are cleared for the next update
            contact_events.clear();
            return;
        }
    }
}

/// Transitions color for player-chaser collisions.
fn transition_color(
    mut query: Query<(&Handle<ColorMaterial>, &mut TimeSinceCollision)>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    time: Res<Time>,
) {
    let Ok((color_handle, mut stopwatch)) = query.get_single_mut() else {
        println!("no color");
        return;
    };
    let material = materials.get_mut(color_handle.id()).unwrap();

    stopwatch.time.tick(time.delta());
    if stopwatch.time.elapsed_secs() > COLOR_TRANSITION_TIME {
        material.color = PLAYER_COLOR.into();
        return;
    }

    let interp_function = |x: f32| x * x * x;

    let progress = stopwatch.time.elapsed_secs() / COLOR_TRANSITION_TIME;
    let s = interp_function(progress);
    material.color = COLLISION_COLOR.mix(&PLAYER_COLOR, s).into();
}
