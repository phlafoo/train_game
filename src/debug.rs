use bevy::{
    color::palettes::{
        basic::AQUA,
        css::{BLUE, GRAY, GREEN, LIME, MAGENTA, RED, YELLOW},
    },
    prelude::*,
    utils::hashbrown::HashMap,
};
use bevy_rapier2d::{
    dynamics::{ExternalForce, ReadMassProperties, Velocity},
    prelude::*,
};

use crate::{
    config::DebugViews,
    spawner::{Spawner, SpawnerTrigger, SpawnerTriggerEvent},
    tilemap::PlayerSpawn,
};

/// Render debug info if enabled in config
pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            (render_objects, render_force_acceleration, render_velocity)
                .after(TransformSystem::TransformPropagate),
        );
    }
}

/// Render acceration due to ExternalForce (not total acceleration) as an arrow
fn render_force_acceleration(
    debug_views: Res<DebugViews>,
    mut gizmos: Gizmos,
    query: Query<(&Transform, &ExternalForce, &ReadMassProperties)>,
) {
    if !debug_views.render_movement {
        return;
    }
    let line_scale = 0.1;

    for (t, f, read_mass) in query.iter() {
        let mass = read_mass.mass;
        let acc = line_scale * f.force / mass;
        let position = t.translation.xy();
        gizmos.arrow_2d(position, acc + position, RED);
    }
}

/// Render velocities as an arrow
fn render_velocity(
    debug_views: Res<DebugViews>,
    mut gizmos: Gizmos,
    query: Query<(&Transform, &Velocity)>,
) {
    if !debug_views.render_movement {
        return;
    }
    let line_scale = 0.3;
    for (t, v) in query.iter() {
        let position = t.translation.xy();
        let velocity = v.linvel * line_scale;
        gizmos.arrow_2d(position, velocity + position, LIME);
    }
}

/// Render triggers, enemy spawners, and player spawn.
fn render_objects(
    debug_views: Res<DebugViews>,
    time: Res<Time>,
    mut spawner_events: EventReader<SpawnerTriggerEvent>,
    q_triggers: Query<(&mut SpawnerTrigger, &Collider, &GlobalTransform)>,
    q_spawners: Query<(&Spawner, &GlobalTransform)>,
    q_player_spawn: Query<&GlobalTransform, With<PlayerSpawn>>,
    mut timers: Local<HashMap<u32, Timer>>,
    mut gizmos: Gizmos,
) {
    if !debug_views.render_objects {
        return;
    }
    /// Radius for rendering enemy and player spawners
    const SPAWNER_RADIUS: f32 = 8.0;
    /// Triggers and enemy spawners will be rendered with these colors
    const SPAWNER_COLORS_ACTIVE: [Srgba; 6] = [AQUA, RED, BLUE, LIME, MAGENTA, YELLOW];

    // When a trigger is not being activated, render with more muted color
    let get_inactive_color = |color: &Srgba| -> Srgba {
        let mut new_color = color.mix(&GRAY, 0.8);
        new_color.set_alpha(0.8);
        new_color
    };

    // Draw player spawn if it exists
    if let Ok(player_spawn_transform) = q_player_spawn.get_single() {
        gizmos.circle_2d(
            player_spawn_transform.translation().xy(),
            SPAWNER_RADIUS,
            GREEN,
        );
    };

    // Map spawner id to (spawner, pos, color) so that connected triggers use the same color
    let mut spawner_map = HashMap::new();
    let mut colors = SPAWNER_COLORS_ACTIVE.iter().cycle();

    // Draw enemy spawners
    for (spawner, spawner_transform) in q_spawners.iter() {
        let color = colors.next().unwrap();
        let pos = spawner_transform.translation().xy();
        spawner_map.insert(spawner.id, (spawner, pos, color));

        let color = if spawner.active {
            *color
        } else {
            get_inactive_color(color)
        };
        gizmos.circle_2d(pos, SPAWNER_RADIUS, color);
    }

    // Spawner triggers that were just triggered
    let activated_triggers = spawner_events.read().map(|e| e.0).collect::<Vec<_>>();

    // Draw trigger zones
    for (trigger, collider, trigger_transform) in q_triggers.iter() {
        // `timers` stores a timer per trigger which determines how long the debug render flashes
        // when the trigger is activated
        let timer = timers.entry(trigger.id()).or_default();
        timer.tick(time.delta());

        // Get rotation and position
        let (_, rotation, position) = trigger_transform.to_scale_rotation_translation();
        let (axis, mut angle) = rotation.to_axis_angle();

        // Rotation is always around z axis. Angle can be flipped depending on z value (-1.0 or 1.0)
        angle *= axis.z;
        let position = position.xy();

        // Determine which color to render with
        let color =
            if let Some(&(spawner, spawner_pos, color)) = spawner_map.get(&trigger.spawner_id) {
                // If this trigger just actived a spawner, start timer to highlight in debug view
                if activated_triggers.contains(trigger)
                    && !spawner.active
                    && spawner.count != spawner.num_spawn
                {
                    *timer = Timer::from_seconds(0.196, TimerMode::Once);
                }

                let color = if timer.finished() {
                    get_inactive_color(color)
                } else {
                    *color
                };
                gizmos.line_2d(position, spawner_pos, color);
                color
            } else {
                // Trigger is not connected to any spawner
                get_inactive_color(colors.next().unwrap())
            };

        // Finally draw the shape
        match collider.as_typed_shape() {
            ColliderView::Cuboid(c) => {
                let size = c.half_extents() * 2.0;
                gizmos.rect_2d(position, angle, size, color);
            }
            ColliderView::Ball(b) => {
                gizmos.circle_2d(position, b.radius(), color);
            }
            ColliderView::ConvexPolygon(p) => {
                let vertices = p.points().collect::<Vec<_>>();
                let poly = BoxedPolygon::new(vertices);
                gizmos.primitive_2d(&poly, position, angle, color);
            }
            _ => (),
        };
    }
}
