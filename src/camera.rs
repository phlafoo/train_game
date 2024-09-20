use bevy::{
    prelude::*,
    render::camera::ScalingMode,
    window::{WindowResized, WindowResolution},
};
use bevy_rapier2d::plugin::PhysicsSet;

use crate::{config::Config, player::Player, tilemap::Tilemap};

/// Plugin that spawns the camera, allows zooming in/out, and has the camera follow the player
pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera)
            // To eliminate jittery camera movement these systems must run AFTER physics and BEFORE transform propogation
            .add_systems(
                PostUpdate,
                (update_range, camera_follow_player)
                    .chain()
                    .after(PhysicsSet::SyncBackend)
                    .before(TransformSystem::TransformPropagate),
            )
            .add_systems(Update, camera_zoom)
            .register_type::<CameraRange>();
        // .insert_resource(Msaa::Sample4);
    }
}

/// Defines valid range (in world coords) for the camera.
/// The goal is for the camera to never show anything outside the tilemap while following the player
/// unless the projection area is larger than the tilemap.
#[derive(Component, Default, Debug, Reflect)]
pub struct CameraRange {
    pub min: Vec3,
    pub max: Vec3,
}

fn update_range(
    mut q_camera: Query<
        (&OrthographicProjection, &mut CameraRange),
        (With<MainCamera>, Changed<OrthographicProjection>),
    >,
    q_map: Query<(&GlobalTransform, &Tilemap)>,
) {
    // Only update range if projection has been changed (zoom in/out or window resized)
    let Ok((ortho, mut range)) = q_camera.get_single_mut() else {
        return;
    };
    let Ok((map_transform, tilemap)) = q_map.get_single() else {
        return;
    };

    // Values are in world units
    let map_width = tilemap.get_physical_width();
    let map_height = tilemap.get_physical_height();
    let cam_width = ortho.area.width();
    let cam_height = ortho.area.height();

    // Set right and left bounds for camera range
    range.max.x = (map_width - cam_width * 0.5).max(map_width * 0.5)
        + map_transform.translation().x
        - map_width * 0.5;
    range.min.x = range.max.x - (map_width - cam_width);

    // Set top and bottom bounds for camera range
    range.max.y = (map_height - cam_height * 0.5).max(map_height * 0.5)
        + map_transform.translation().y
        - map_height * 0.5;
    range.min.y = range.max.y - (map_height - cam_height);
}

/// Marker component used to identify the main camera
#[derive(Component)]
pub struct MainCamera;

fn spawn_camera(mut commands: Commands, q_window: Query<&Window>) {
    let mut camera = Camera2dBundle::default();

    let res = &q_window.single().resolution;
    camera.projection.scaling_mode = get_scaling_mode(res);

    commands.spawn((camera, CameraRange::default(), MainCamera));
}

fn get_scaling_mode(res: &WindowResolution) -> ScalingMode {
    /// Having a physical screen height that matches this number will result in perfect pixel mapping with the tilemap texture.
    /// Larger values zoom the camera out. 1361 is the height in pixels of a maximized window on my monitor (1.0 window scale).
    const SCREEN_HEIGHT_WORLD_UNITS: f32 = 1361.;

    // For some reason I could not get ScalingMode::FixedVertical to have perfect pixel mapping with tilemap texture
    let pixels_per_world_unit = res.physical_height() as f32 / SCREEN_HEIGHT_WORLD_UNITS;
    ScalingMode::WindowSize(pixels_per_world_unit)
}

fn camera_zoom(
    time: Res<Time>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut q_camera: Query<&mut OrthographicProjection, With<MainCamera>>,
    q_window: Query<&Window>,
    resize_reader: EventReader<WindowResized>,
) {
    /// Min scale for orthographic projection
    const MIN_SCALE: f32 = 0.03;

    let mut ortho = q_camera.single_mut();

    // Zoom in/out faster when key is pressed
    let scale_inc = if keyboard_input.pressed(KeyCode::ControlLeft) {
        4.0
    } else {
        0.05
    };
    // Reset scale
    if keyboard_input.just_pressed(KeyCode::Digit0) {
        ortho.scale = 1.0;
    }
    // Zoom in
    if keyboard_input.pressed(KeyCode::Equal) {
        ortho.scale -= scale_inc * time.delta_seconds();
    }
    // Zoom out
    if keyboard_input.pressed(KeyCode::Minus) {
        ortho.scale += scale_inc * time.delta_seconds();
    }
    // Limit zoom
    if ortho.scale < MIN_SCALE {
        ortho.scale = MIN_SCALE;
    }
    // Update scaling mode if window was resized
    if !resize_reader.is_empty() {
        let res = &q_window.single().resolution;
        ortho.scaling_mode = get_scaling_mode(res);
    }
}

/// Adjust camera position to track player
fn camera_follow_player(
    mut q_camera: Query<(&mut Transform, &CameraRange, &OrthographicProjection), With<MainCamera>>,
    q_player: Query<(&Transform, &Player), Without<MainCamera>>,
    time: Res<Time>,
    config: Res<Config>,
) {
    /// Camera will not move if it is within this distance of the player
    const MIN_DIFF: f32 = 10.0;
    /// Distance from the map border that the camera will start to slow down
    const CUSHION_DIST: f32 = 80.0;
    /// If follow distance is too low the camera movement becomes glitchy
    const MIN_FOLLOW_DIST: f32 = 10.0;

    // Get player info
    let Ok((player_transform, player)) = q_player.get_single() else {
        return;
    };
    // Get camera info
    let (mut transform, range, ortho) = q_camera.single_mut();
    let translation = &mut transform.translation;

    // Camera speed will depend on how far away the player is
    let mut diff = (player_transform.translation - *translation).xy();
    // Prevents camera movement if player is within `MIN_DIFF` distance of camera
    diff = (diff.abs() - MIN_DIFF).max(Vec2::ZERO).copysign(diff);

    // Stay closer to player (in world units) if zoomed in
    let max_follow_dist = (config.camera_follow_dist * ortho.scale).max(MIN_FOLLOW_DIST);

    // Scale such that the player will not exceed `max_follow_dist` distance from the center of
    // the screen while moving at `boost_max_speed`
    diff *= player.boost_max_speed / max_follow_dist;

    let mut dx = diff.x;
    let mut dy = diff.y;

    // Distance from the bottom left of the camera's allowable area
    let dist_to_min = *translation - range.min;
    // Distance from the top right of the camera's allowable area
    let dist_to_max = range.max - *translation;

    // Scale down the max velocity of the camera in proportion with distance to border
    if dist_to_min.x < CUSHION_DIST {
        dx = dx.max(dx * (dist_to_min.x / CUSHION_DIST));
    }
    if dist_to_max.x < CUSHION_DIST {
        dx = dx.min(dx * (dist_to_max.x / CUSHION_DIST));
    }
    if dist_to_min.y < CUSHION_DIST {
        dy = dy.max(dy * (dist_to_min.y / CUSHION_DIST));
    }
    if dist_to_max.y < CUSHION_DIST {
        dy = dy.min(dy * (dist_to_max.y / CUSHION_DIST));
    }
    // Update camera position
    translation.x += dx * time.delta_seconds();
    translation.y += dy * time.delta_seconds();

    // Clamp to allowable area
    *translation = translation.clamp(range.min, range.max);
}
