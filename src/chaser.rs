use bevy::{prelude::*, sprite::Mesh2dHandle};
use bevy_rapier2d::prelude::*;
use bevy_svg::prelude::*;

use crate::{
    config::Config,
    flowfield::apply_force,
    physics::{CHASER_GROUP, WALL_GROUP},
    player::Player,
};

const CHASER_ACCELERATION: f32 = 8000.;
pub const CHASER_RADIUS: f32 = 7.;
pub const CHASER_BORDER_THICKNESS: f32 = 2.;
const CHASER_MAX_SPEED: f32 = 200.0;
const CHASER_HANDLING: f32 = 0.2; // 0..1
const CHASER_COLOR: Color = Color::srgb(1.0, 0.5, 0.0);
const CHASER_DAMPING: f32 = 5.0;

pub struct ChaserPlugin;

impl Plugin for ChaserPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup)
            .add_systems(Update, update_chaser_avoidance.after(apply_force));
        // .add_systems(Update, update_chaser_velocity);
    }
}

/// Marker
#[derive(Component, Debug, Clone)]
pub struct Chaser;

#[derive(Bundle, Clone)]
pub struct ChaserBundle {
    // pub sprite_bundle: SpriteBundle,
    // pub svg_bundle: Svg2dBundle,
    pub collider: Collider,
    pub groups: CollisionGroups,
    pub rigid_body: RigidBody,
    pub soft_ccd: SoftCcd,
    pub force: ExternalForce,
    pub damping: Damping,
    pub velocity: Velocity,
    pub friction: Friction,
    pub restitution: Restitution,
    pub read_mass_properties: ReadMassProperties,
    pub collider_mass_properties: ColliderMassProperties,
}

impl Default for ChaserBundle {
    fn default() -> Self {
        Self {
            // sprite_bundle: SpriteBundle::default(),
            rigid_body: RigidBody::Dynamic,
            groups: CollisionGroups::new(CHASER_GROUP, Group::ALL),
            soft_ccd: SoftCcd { prediction: 8.0 },
            collider: Collider::ball(CHASER_RADIUS),
            collider_mass_properties: ColliderMassProperties::Mass(30.),
            restitution: Restitution::coefficient(0.7),
            damping: Damping {
                linear_damping: CHASER_DAMPING,
                angular_damping: 0.5,
            },
            friction: Friction {
                coefficient: 0.0,
                combine_rule: CoefficientCombineRule::Min,
            },
            force: Default::default(),
            // material_bundle: Default::default(),
            velocity: Default::default(),
            read_mass_properties: Default::default(),
        }
    }
}

#[derive(Resource)]
#[allow(dead_code)]
pub struct ChaserAssets {
    pub svg: Handle<Svg>,
    pub texture: Handle<Image>,
    pub mesh_circle: Mesh2dHandle,
    pub mesh_border: Mesh2dHandle,
    pub material_circle: Handle<ColorMaterial>,
    pub material_border: Handle<ColorMaterial>,
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    commands.insert_resource(ChaserAssets {
        svg: asset_server.load("svgs/chaser.svg"),
        texture: asset_server.load("sprites/chaser.png"),
        mesh_circle: Mesh2dHandle(meshes.add(Circle {
            radius: CHASER_RADIUS,
        })),
        material_circle: materials.add(ColorMaterial::from(CHASER_COLOR)),
        mesh_border: Mesh2dHandle(meshes.add(Annulus::new(
            CHASER_RADIUS - CHASER_BORDER_THICKNESS,
            CHASER_RADIUS,
        ))),
        material_border: materials.add(ColorMaterial::from_color(Color::BLACK)),
    });
}

/// Basic follow player with no pathfinding
#[allow(unused)]
fn chasers_follow_player(
    mut query_chasers: Query<(&mut Velocity, &Transform), With<Chaser>>,
    query_player: Query<&Transform, With<Player>>,
    time: Res<Time>,
) {
    let player_translation = query_player.single().translation;
    let dt = time.delta_seconds();
    for (mut velocity, transform) in query_chasers.iter_mut() {
        let mut new_velocity = player_translation.xy() - transform.translation.xy();
        let v = &mut velocity.linvel;
        let mag_before = v.length();

        new_velocity = new_velocity.normalize_or_zero();

        let mag = if mag_before > CHASER_MAX_SPEED {
            mag_before - 1. / CHASER_HANDLING * 100. * dt
        } else {
            mag_before + 1. / CHASER_HANDLING * CHASER_ACCELERATION * dt
        };

        new_velocity *= mag;

        *v = v.lerp(new_velocity, CHASER_HANDLING);

        if mag_before <= CHASER_MAX_SPEED {
            *v = v.clamp_length_max(CHASER_MAX_SPEED - 0.0001);
        }
    }
}

fn update_chaser_avoidance(
    mut query_chasers: Query<(&mut ExternalForce, &Transform), With<Chaser>>,
    config: Res<Config>,
    rapier_context: Res<RapierContext>,
) {
    let filter = QueryFilter::new().groups(CollisionGroups::new(
        CHASER_GROUP | WALL_GROUP,
        CHASER_GROUP | WALL_GROUP,
    ));

    for (mut ext_force, transform) in query_chasers.iter_mut() {
        let pos = transform.translation.xy();

        if let Some((_, projection)) = rapier_context.project_point(pos, false, filter) {
            let avoidance = pos - projection.point;
            let length_sq = avoidance.length_squared();

            let avoidance_magnitude =
                (length_sq.recip() * config.chaser_avoidance_mul).min(config.chaser_avoidance_max);

            // Set length of `avoidance` to `avoidance_magnitude`
            ext_force.force += avoidance_magnitude * (avoidance / length_sq.sqrt());
            ext_force.force = ext_force.force.clamp_length_max(40000.0);
        }
    }
}
