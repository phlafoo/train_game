use bevy::prelude::*;
use bevy_rapier2d::prelude::*;

use crate::config::DebugViews;

/// Add rapier physics
pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0))
            .add_plugins(RapierDebugRenderPlugin::default().disabled())
            .add_systems(Startup, disable_gravity)
            .add_systems(Update, toggle_rapier_debug);
    }
}

pub const WALL_GROUP: Group = Group::GROUP_1;
pub const TRIGGER_GROUP: Group = Group::GROUP_2;
pub const PLAYER_GROUP: Group = Group::GROUP_3;
pub const CHASER_GROUP: Group = Group::GROUP_4;

fn disable_gravity(mut rapier_config: ResMut<RapierConfiguration>) {
    rapier_config.gravity = Vect::ZERO;
}

/// Toggle debug renderer
fn toggle_rapier_debug(
    config: Res<DebugViews>,
    mut debug_render_context: ResMut<DebugRenderContext>,
) {
    debug_render_context.enabled = config.render_rapier;
}
