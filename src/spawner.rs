use bevy::prelude::*;
use bevy_rapier2d::{prelude::*, rapier::prelude::CollisionEventFlags};
use bevy_svg::prelude::*;
use rand::Rng;
use std::{f32::consts::PI, time::Duration};
use tiled::{ObjectData, PropertyValue};

use crate::{
    chaser::{Chaser, ChaserAssets, ChaserBundle},
    config::Config,
};

pub struct SpawnPlugin;

impl Plugin for SpawnPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, handle_trigger_event)
            .add_systems(Update, (update_spawners, remove_chasers).chain())
            .add_event::<SpawnerTriggerEvent>();
    }
}

/// Implemented on [`tiled::ObjectData`] to get custom property values
trait CustomProperties {
    fn get_bool(&self, key: impl AsRef<str>) -> Option<bool>;
    fn get_i32(&self, key: impl AsRef<str>) -> Option<i32>;
    fn get_u32(&self, key: impl AsRef<str>) -> Option<u32>;
    fn get_f32(&self, key: impl AsRef<str>) -> Option<f32>;
}

impl CustomProperties for ObjectData {
    fn get_bool(&self, key: impl AsRef<str>) -> Option<bool> {
        if let &PropertyValue::BoolValue(value) = self.properties.get(key.as_ref())? {
            return Some(value);
        }
        None
    }
    fn get_i32(&self, key: impl AsRef<str>) -> Option<i32> {
        if let &PropertyValue::IntValue(value) = self.properties.get(key.as_ref())? {
            return Some(value);
        }
        None
    }
    fn get_u32(&self, key: impl AsRef<str>) -> Option<u32> {
        if let &PropertyValue::ObjectValue(value) = self.properties.get(key.as_ref())? {
            return Some(value);
        }
        None
    }
    fn get_f32(&self, key: impl AsRef<str>) -> Option<f32> {
        if let &PropertyValue::FloatValue(value) = self.properties.get(key.as_ref())? {
            return Some(value);
        }
        None
    }
}

#[derive(Component, Default, Reflect, Debug)]
pub struct Spawner {
    pub id: u32,
    pub active: bool,
    pub active_default: bool,
    pub num_spawn: i32,
    pub delay: f32,
    pub immediate: bool,
    pub interval: f32,
    pub repeats: bool,
    pub count: i32,
    pub timer: Timer,
}

impl Spawner {
    pub fn from_object(object_data: &ObjectData) -> Self {
        // Get custom properties
        let active_default = object_data.get_bool("active").unwrap();
        let num_spawn = object_data.get_i32("num_spawn").unwrap();
        let delay = object_data.get_f32("delay").unwrap();
        let immediate = object_data.get_bool("immediate").unwrap();
        let interval = object_data.get_f32("interval").unwrap();
        let repeats = object_data.get_bool("repeats").unwrap();

        // Setup timer
        let duration = if immediate { 0.0 } else { delay };
        let timer = Timer::from_seconds(duration, TimerMode::Repeating);

        Spawner {
            id: object_data.id(),
            active: active_default,
            active_default,
            num_spawn,
            delay,
            immediate,
            interval,
            repeats,
            count: 0,
            timer,
        }
    }
}

#[derive(Component, Default, Reflect, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnerTrigger {
    /// object id of this trigger (only used for debug view atm)
    id: u32,
    /// Which spawner this trigger will activate
    pub spawner_id: u32,
}

impl SpawnerTrigger {
    #[inline(always)]
    pub fn from_object(object_data: &ObjectData) -> Self {
        let spawner_id = object_data.get_u32("spawner_id").unwrap();
        SpawnerTrigger {
            id: object_data.id(),
            spawner_id,
        }
    }

    #[inline(always)]
    pub fn id(&self) -> u32 {
        self.id
    }
}

/// Removes chasers when max_chasers is reduced
fn remove_chasers(
    mut commands: Commands,
    config: Res<Config>,
    chaser_query: Query<Entity, With<Chaser>>,
) {
    let delete_count = chaser_query.iter().count() as isize - config.max_chasers as isize;
    for (i, entity) in chaser_query.iter().enumerate() {
        if i as isize >= delete_count {
            return;
        }
        commands.entity(entity).despawn_recursive();
    }
}

#[derive(Event)]
pub struct SpawnerTriggerEvent(pub SpawnerTrigger);

// TODO spawn chasers as a child of some entity to not clog world inspector ui
/// Spawns chasers
fn update_spawners(
    mut commands: Commands,
    config: Res<Config>,
    chaser_assets: Res<ChaserAssets>,
    time: Res<Time>,
    mut spawner_events: EventReader<SpawnerTriggerEvent>,
    chasers: Query<&Chaser>,
    mut q_spawners: Query<(&mut Spawner, &GlobalTransform)>,
) {
    // TODO store chaser count somewhere so it don't need to be recalculated per frame
    if chasers.iter().count() >= config.max_chasers {
        return;
    }

    // Get list of spawners which were just triggered
    let spawner_ids = spawner_events
        .read()
        .map(|e| e.0.spawner_id)
        .collect::<Vec<_>>();

    let mut rng = rand::thread_rng();

    for (mut spawner, transform) in q_spawners.iter_mut() {
        // Activate spawner
        if !spawner.active && spawner.count < spawner.num_spawn && spawner_ids.contains(&spawner.id)
        {
            spawner.active = true;
        }
        // If spawner is active, spawn chaser if timer just finished
        if spawner.active {
            spawner.timer.tick(time.delta());

            // TODO comments?
            if spawner.timer.just_finished() {
                spawner.count += 1;
                if spawner.count == spawner.num_spawn {
                    if spawner.repeats {
                        spawner.count = 0;
                        let duration = (spawner.delay - spawner.timer.elapsed_secs()).max(0.0);
                        spawner
                            .timer
                            .set_duration(Duration::from_secs_f32(duration));
                    } else {
                        spawner.active = false;
                    }
                } else {
                    let duration = spawner.interval - spawner.timer.elapsed_secs();
                    spawner
                        .timer
                        .set_duration(Duration::from_secs_f32(duration));
                }

                let angle = rng.gen_range(-PI..PI);
                let rotation = Quat::from_rotation_z(angle);

                commands.spawn((
                    ChaserBundle::default(),
                    Svg2dBundle {
                        svg: chaser_assets.svg.clone(),
                        origin: Origin::TopLeft,
                        transform: Transform {
                            translation: transform.translation(),
                            scale: Vec3::ONE,
                            rotation,
                        },
                        ..default()
                    },
                    Chaser,
                    Name::new("Chaser"),
                ));
            }
        }
    }
}

fn handle_trigger_event(
    mut spawner_events: EventWriter<SpawnerTriggerEvent>,
    mut collision_events: EventReader<CollisionEvent>,
    q_trigger: Query<&SpawnerTrigger, With<Sensor>>,
) {
    for event in collision_events.read() {
        // Only check SENSOR colliders (trigger has sensor)
        let &CollisionEvent::Started(collider1, collider2, CollisionEventFlags::SENSOR) = event
        else {
            continue;
        };
        // If collider is not a trigger, skip
        let Ok(&trigger) = q_trigger
            .get(collider1)
            .or_else(|_| q_trigger.get(collider2))
        else {
            continue;
        };
        spawner_events.send(SpawnerTriggerEvent(trigger));
    }
}
