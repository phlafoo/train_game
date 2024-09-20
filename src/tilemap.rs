use bevy::{
    asset::AssetPath,
    math::{uvec2, vec2, vec3},
    prelude::*,
    utils::hashbrown::{HashMap, HashSet},
};
use bevy_fast_tilemap::prelude::*;
use bevy_rapier2d::prelude::*;
use clap::Parser;
use std::{f32::consts::TAU, path::PathBuf};
use tiled::{Loader, TileId, TileLayer};

use crate::{
    cursor::MyWorldCoords,
    flowfield::*,
    physics::{PLAYER_GROUP, TRIGGER_GROUP, WALL_GROUP},
    point::Point,
    segment::Segment,
    spawner::{Spawner, SpawnerTrigger},
};

const EMPTY_TILE_ID: TileId = 5;

const WALL_LAYER: &str = "wall layer";
const OBJECT_LAYER: &str = "object layer";
const PLAYER_SPAWN: &str = "PlayerSpawn";
const ENEMY_SPAWNER: &str = "Spawner";
const SPAWNER_TRIGGER: &str = "SpawnerTrigger";

pub struct MyTilemapPlugin;

impl Plugin for MyTilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (load_tilemap, print_vertex_count).chain())
            .init_resource::<Args>();
        // .add_systems(Update, print_tile_coords)
        // .add_systems(Update, get_tile_at_cursor)
    }
}

#[derive(Component, Default)]
pub struct Tilemap {
    /// width in number of tiles
    pub width: usize,
    /// height in number of tiles
    pub height: usize,
    // TODO consider u32 for tile dim
    /// width of a tile in pixels
    pub tile_width: f32,
    /// height of a tile in pixels
    pub tile_height: f32,
}

impl Tilemap {
    pub fn get_physical_width(&self) -> f32 {
        self.width as f32 * self.tile_width
    }
    pub fn get_physical_height(&self) -> f32 {
        self.height as f32 * self.tile_height
    }
    pub fn tile_to_world_coords(&self, tile_coords: (u32, u32)) -> Point {
        Point::new(
            (tile_coords.0 as f32 + 0.5) * self.tile_width,
            (tile_coords.1 as f32 + 0.5) * self.tile_height,
        )
    }

    pub fn world_to_tile_coords(&self, world_coords: &Vec2) -> Vec2 {
        vec2(
            (world_coords.x / self.tile_width).clamp(0.0, self.width as f32 - 1.),
            (world_coords.y / self.tile_height).clamp(0.0, self.height as f32 - 1.),
        )
    }
}

#[derive(Bundle, Default)]
pub struct TilemapBundle {
    tilemap: Tilemap,
    storage: TileStorage,
    pub transform: Transform,
    pub global_transform: GlobalTransform,
    pub inherited_visibility: InheritedVisibility,
}

/// Stores the tile index that the cursor is on
#[derive(Resource, Default)]
pub struct MyTileCoords(pub Vec2);

#[derive(Parser, Debug, Resource, Default)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// map filename
    #[arg(short, long)]
    pub map: String,
    /// perform benchmark
    #[arg(short, long)]
    pub bench: bool,
}

#[derive(Component, Default, Reflect, Debug)]
pub struct PlayerSpawn;

#[derive(Component, Default)]
pub struct TileStorage(pub Vec<u8>);

/// Marker
#[derive(Component)]
pub struct WallCollider;

// TODO refactor!
fn load_tilemap(
    mut commands: Commands,
    mut materials: ResMut<Assets<Map>>,
    mut args: ResMut<Args>,
    asset_server: Res<AssetServer>,
) {
    println!("Loading tilemap...");
    const FRONT_LAYER_Z: f32 = -20.0;

    *args = Args::parse();

    let map_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
        .join(format!("assets/levels/{}", args.map));

    let mut loader = Loader::new();
    let map = loader.load_tmx_map(map_path).unwrap();
    let mut tileset = None;
    let layer_count = map.layers().len();
    let first_layer_z = FRONT_LAYER_Z - layer_count as f32;

    let mut tilemap = Tilemap::default();
    let mut storage = TileStorage::default();
    let mut map_translation = Vec3::ZERO;
    let mut map_layers = vec![];
    let mut colliders: Vec<Collider> = vec![];

    for (layer_index, layer) in map.layers().enumerate() {
        print!("Layer \"{}\":\n\t", layer.name);

        match layer.layer_type() {
            tiled::LayerType::Tiles(tile_layer) => {
                let TileLayer::Finite(data) = tile_layer else {
                    panic!("Infinite maps not supported!");
                };
                let width = data.width();
                let height = data.height();

                println!(
                    "Finite tile layer with width = {} and height = {}; ID of tile @ (0,0): {}",
                    width,
                    height,
                    data.get_tile(0, 0).map_or(EMPTY_TILE_ID, |t| t.id()),
                );
                if tileset.is_none() {
                    tileset = (0..width)
                        .zip(0..height)
                        .find_map(|(x, y)| data.get_tile(x as i32, y as i32))
                        .map(|t| t.get_tileset());
                }
                let Some(tileset) = tileset else {
                    println!(
                        "Failed to get tileset from layer \"{}\". Maybe the layer is empty?",
                        layer.name
                    );
                    continue;
                };
                let tile_width = tileset.tile_width as f32;
                let tile_height = tileset.tile_height as f32;

                let tileset_path = &tileset
                    .image
                    .as_ref()
                    .expect("tileset should have an image")
                    .source;

                // TODO swap all tilemaps to use resized 256x256 texture atlas
                let spacing = Vec2::splat(tileset.spacing as f32);
                let margin = Vec2::splat(tileset.margin as f32);

                // Create map
                let map = Map::builder(
                    // Map size (tiles)
                    uvec2(width, height),
                    // Tile atlas
                    // asset_server.load(AssetPath::parse("tilemap16x-nopadding.png")),
                    asset_server.load(AssetPath::from_path(tileset_path)),
                    // Tile size (pixels)
                    vec2(tile_width, tile_height),
                )
                .with_padding(spacing, margin, margin)
                .build_and_set(|UVec2 { x, y }| {
                    data.get_tile(x as i32, y as i32)
                        .map_or(EMPTY_TILE_ID, |t| t.id())
                });

                if layer_index == 0 {
                    map_translation.x = width as f32 * tile_width * 0.5;
                    map_translation.y = height as f32 * tile_height * 0.5;
                    tilemap = Tilemap {
                        width: width as usize,
                        height: height as usize,
                        tile_width,
                        tile_height,
                    };
                }
                map_layers.push((
                    MapBundleManaged {
                        material: materials.add(map),
                        transform: Transform::from_translation(vec3(
                            0.0,
                            0.0,
                            first_layer_z + layer_index as f32,
                        )),
                        ..default()
                    },
                    Name::new(layer.name.clone()),
                ));
                if layer.name == WALL_LAYER {
                    println!("creating storage");

                    let mut segments: HashSet<Segment> = HashSet::new();
                    let mut points_map = HashMap::new();

                    for y in (0..height as i32).rev() {
                        for x in 0..width as i32 {
                            let pos: Point =
                                tilemap.tile_to_world_coords((x as u32, height - 1 - y as u32));

                            let bitmask = match data.get_tile_data(x, y).map(|t| t.id()) {
                                Some(id) => {
                                    let tile = tileset.get_tile(id).unwrap();
                                    if let Some(object_data) =
                                        tile.collision.as_ref().map(|c| &c.object_data()[0])
                                    {
                                        let new_segments = match object_data.shape.clone() {
                                            tiled::ObjectShape::Rect { width, height } => {
                                                let half_x = width * 0.5;
                                                let half_y = height * 0.5;
                                                let p = [
                                                    pos + Point::new(-half_x, half_y),
                                                    pos + Point::new(-half_x, -half_y),
                                                    pos + Point::new(half_x, -half_y),
                                                    pos + Point::new(half_x, half_y),
                                                ];
                                                vec![
                                                    Segment::new(p[0], p[1]),
                                                    Segment::new(p[1], p[2]),
                                                    Segment::new(p[2], p[3]),
                                                    Segment::new(p[3], p[0]),
                                                ]
                                            }
                                            tiled::ObjectShape::Polygon { points } => {
                                                // let points = points.iter().cycle().take(points.len() + 1);
                                                // let clockwise = false;

                                                let points = points.iter().map(|&(x, y)| {
                                                    pos + Point::new(
                                                        x - object_data.x - tile_width * 0.5,
                                                        -y - object_data.y + tile_height * 0.5,
                                                    )
                                                });

                                                // Make sure shape is counter-clockwise
                                                let sum = points
                                                    .clone()
                                                    .zip(points.clone().cycle().skip(1))
                                                    .fold(0.0, |acc, (p1, p2)| {
                                                        acc + (p2.x - p1.x) * (p2.y + p1.y)
                                                    });
                                                let points: Vec<Point> = if sum > 0.0 {
                                                    // println!(" COUNTER CLOCKWISE. sum: {sum}");
                                                    points.rev().collect()
                                                } else {
                                                    points.collect()
                                                };

                                                let new_segments = points
                                                    .iter()
                                                    .zip(points.iter().cycle().skip(1))
                                                    .map(|(&p1, &p2)| Segment::new(p1, p2))
                                                    .collect::<Vec<_>>();

                                                new_segments
                                            }
                                            _ => vec![],
                                        };
                                        for &new_segment in new_segments.iter() {
                                            if segments.contains(&new_segment) {
                                                let v: &mut Vec<Segment> =
                                                    points_map.get_mut(&new_segment.b).unwrap();
                                                if v.len() <= 1 {
                                                    points_map.remove(&new_segment.b);
                                                } else {
                                                    let index = v
                                                        .iter()
                                                        .position(|seg| *seg == new_segment)
                                                        .unwrap();
                                                    v.swap_remove(index);
                                                }
                                                segments.remove(&new_segment);
                                                continue;
                                            }
                                            points_map
                                                .entry(new_segment.a)
                                                .and_modify(|v: &mut Vec<Segment>| {
                                                    if !segments.contains(&new_segment) {
                                                        v.push(new_segment);
                                                    }
                                                })
                                                .or_insert(vec![new_segment]);
                                            segments.insert(new_segment);
                                        }
                                    }
                                    match id {
                                        // triangle wall tiles
                                        19 => NW_SUBTILE_BITMASK,
                                        18 => NE_SUBTILE_BITMASK,
                                        4 => SW_SUBTILE_BITMASK,
                                        3 => SE_SUBTILE_BITMASK,
                                        // half wall tiles
                                        31 => N_SUBTILE_BITMASK,
                                        15 => E_SUBTILE_BITMASK,
                                        1 => S_SUBTILE_BITMASK,
                                        17 => W_SUBTILE_BITMASK,
                                        // normal wall tile
                                        16 => WALL_BITMASK,
                                        // air tile, spawner
                                        _ => 0,
                                    }
                                }
                                None => 0,
                            };
                            storage.0.push(bitmask);
                        }
                    }

                    while let Some((&start_point, start_segments)) = points_map.iter_mut().next() {
                        let mut s = start_segments.swap_remove(0);
                        if start_segments.is_empty() {
                            points_map.remove(&start_point);
                        }

                        let mut vertices = vec![];
                        vertices.push(s.a);
                        let first_slope = s.get_slope_correlate();
                        let mut slope = first_slope;

                        loop {
                            let Some(segs) = points_map.get_mut(&s.b) else {
                                panic!("End point of segment not in points map!  s: {:?}", s);
                            };
                            if segs.is_empty() {
                                panic!("Point should have associated segments!  {:?}", s.b);
                            }
                            let index = if segs.len() == 1 {
                                0
                            } else {
                                let mut min_angle = f32::MAX;
                                let mut min_index = 0;
                                for (index, seg) in segs.iter().enumerate() {
                                    let seg_rev = seg.reverse();
                                    let mut angle = s.angle_between(&seg_rev);
                                    if angle < 0.0 {
                                        angle += TAU;
                                    }
                                    // println!("  seg: {:?}, angle: {angle}", seg);
                                    if angle < min_angle {
                                        min_angle = angle;
                                        min_index = index;
                                    }
                                }
                                min_index
                            };

                            s = segs.swap_remove(index);
                            if segs.is_empty() {
                                // println!("  empty, s.ps: {:?}", s.p1);
                                points_map.remove(&s.a);
                            }

                            // s = points_map.get(&s.p2).map(|v| v[0]).unwrap();
                            let next_slope = s.get_slope_correlate();

                            if slope != next_slope {
                                slope = next_slope;
                                vertices.push(s.a);
                            }

                            if vertices.contains(&s.b) && points_map.get(&s.b).is_none() {
                                if first_slope == slope {
                                    // Remove first vertex since it is redundant
                                    vertices.swap_remove(0);
                                }
                                break;
                            }
                        }

                        // let beveled = vec![];
                        // for i in 1..vertices.len() {
                        //     let s1 = Segment::new(vertices[i - 1], vertices[i]);
                        //     let s2 = Segment::new(vertices[i], vertices[(i + 1) % vertices.len()]);

                        // }
                        let mut index = 0_u32;
                        let (a, b) = (vertices[0], vertices[1]);
                        let mut s1 = Segment::new(a, b);

                        let (vertices, indices): (Vec<_>, Vec<_>) = vertices
                            .iter()
                            .zip(vertices.iter().cycle().skip(1))
                            .zip(vertices.iter().cycle().skip(2))
                            .flat_map(|((&a, &b), &c)| {
                                let s2 = Segment::new(b, c);
                                let angle = s1.angle_between(&s2);
                                s1 = s2;
                                if angle > 0.0 {
                                    // convex
                                    /// Max axis offset
                                    const MAX: f32 = 0.3;

                                    index += 2;

                                    let d1 = (b - a).clamp_axes(MAX);
                                    let d2 = (c - b).clamp_axes(MAX);
                                    vec![(b - d1, None), (b + d2, Some([index - 1, index]))]
                                } else {
                                    // concave
                                    index += 1;
                                    vec![(b, Some([index - 1, index]))]
                                }
                            })
                            .unzip();

                        let indices = indices.into_iter().flatten().collect::<Vec<_>>();

                        let vertices = vertices
                            .iter()
                            .map(|p| p.as_vec2())
                            .cycle()
                            .take(vertices.len() + 1)
                            .collect::<Vec<_>>();

                        // colliders.push(Collider::polyline(vertices, None));
                        colliders.push(Collider::polyline(vertices, Some(indices)));
                    }
                }
            }
            tiled::LayerType::Objects(object_layer) => {
                // object layer contains player and enemy spawners and trigger zones
                println!("Object layer has {} objects", object_layer.objects().len());
                if layer.name != OBJECT_LAYER {
                    continue;
                }
                // process objects
                for object_data in object_layer.object_data().iter() {
                    let translation = vec3(
                        object_data.x,
                        tilemap.get_physical_height() - object_data.y,
                        0.0,
                    );
                    let mut transform = Transform::from_translation(translation);

                    match object_data.user_type.as_str() {
                        PLAYER_SPAWN => {
                            commands
                                .spawn((PlayerSpawn, TransformBundle::from_transform(transform)));
                        }
                        ENEMY_SPAWNER => {
                            commands.spawn((
                                Spawner::from_object(object_data),
                                TransformBundle::from_transform(
                                    transform
                                        .with_translation(translation.with_z(FRONT_LAYER_Z - 2.5)),
                                ),
                            ));
                        }
                        SPAWNER_TRIGGER => {
                            println!("{:#?}", object_data);
                            let angle = f32::to_radians(-object_data.rotation);
                            let rotation = Quat::from_rotation_z(angle);
                            transform.rotate(rotation);

                            // This is required because in Tiled, shapes are anchored at their top
                            // left corner while in Bevy they are anchored at their center.
                            let update_transform =
                                |translation: &mut Vec3, half_x: f32, half_y: f32| {
                                    // north west corner
                                    let nw_corner = vec2(-half_x, half_y);
                                    // north west corner after rotation
                                    let new_nw_corner = Rot2::radians(angle) * nw_corner;

                                    let offset_rotation = nw_corner - new_nw_corner;
                                    let offset = vec3(half_x, -half_y, 0.0);

                                    *translation += offset_rotation.extend(0.0) + offset;
                                };

                            let collider = match object_data.shape {
                                tiled::ObjectShape::Rect { width, height } => {
                                    let half_x = width * 0.5;
                                    let half_y = height * 0.5;

                                    update_transform(&mut transform.translation, half_x, half_y);
                                    Collider::cuboid(half_x, half_y)
                                }
                                tiled::ObjectShape::Ellipse { width, height } => {
                                    let half_x = width * 0.5;
                                    let half_y = height * 0.5;

                                    update_transform(&mut transform.translation, half_x, half_y);

                                    // There is no ellipse collider but we can create a ball and
                                    // then set the scale which will convert it into a elliptical
                                    // polygon if necessary.
                                    let mut ellipse = Collider::ball(half_x);

                                    let ratio = height / width;
                                    println!("height: {height}, width: {width}, ratio: {ratio}");
                                    if ratio != 1.0 {
                                        // circle -> ellipse
                                        transform.scale = vec3(1.0, ratio, 1.0);
                                        ellipse.set_scale(vec2(1.0, ratio), 20);
                                    }
                                    ellipse
                                }
                                _ => panic!(
                                    "Trigger shape not supported. Object ID: {}",
                                    object_data.id()
                                ),
                            };
                            commands.spawn((
                                SpawnerTrigger::from_object(object_data),
                                collider,
                                CollisionGroups::new(TRIGGER_GROUP, PLAYER_GROUP),
                                Sensor,
                                TransformBundle::from_transform(transform),
                            ));
                        }
                        _ => (),
                    }
                }
            }
            tiled::LayerType::Image(_) => panic!("Image layers not supported!"),
            tiled::LayerType::Group(_) => panic!("Group layers not supported!"),
        }
    }
    commands
        .spawn((
            TilemapBundle {
                tilemap,
                storage,
                transform: Transform::from_translation(map_translation),
                ..default()
            },
            Name::new("Tilemap"),
        ))
        .with_children(|parent: &mut ChildBuilder| {
            let mut wall_layer = None;

            for (map, name) in map_layers {
                let name_str = name.to_string();
                // let layer = parent.spawn((map, name));
                wall_layer = if name_str == WALL_LAYER {
                    Some(parent.spawn((map, name)))
                } else {
                    parent.spawn((map, name));
                    None
                };
            }
            let Some(mut wall_layer) = wall_layer else {
                return;
            };
            // let vec = colliders
            //     .into_iter()
            //     .map(|(c, _)| {
            //         (
            //             c.transform_bundle.local.translation.xy(),
            //             0.0_f32,
            //             c.collider,
            //         )
            //     })
            //     .collect::<Vec<_>>();

            // wall_layer.with_children(|parent_layer| {
            //     parent_layer.spawn((Collider::compound(vec), Name::new("Colliders")));
            // });

            wall_layer.with_children(|parent_layer| {
                for c in colliders.into_iter() {
                    parent_layer.spawn((
                        c,
                        WallCollider,
                        Friction {
                            coefficient: 0.0,
                            combine_rule: CoefficientCombineRule::Min,
                        },
                        CollisionGroups::new(WALL_GROUP, Group::ALL),
                    ));
                }
            });
        });
}

fn print_vertex_count(collider: Query<&Collider>) {
    let collider_count = collider.iter().count();
    let vertex_count = collider.iter().fold(0, |acc, c| {
        acc + c.as_polyline().map(|p| p.raw.vertices().len()).unwrap_or(0)
    });

    let avg_vertices = vertex_count as f32 / collider_count as f32;

    println!(
        "Tilemap collision data processed\n * Total vertices: {}\n * Total colliders: {}\n * Average vertices: {}",
        vertex_count, collider_count, avg_vertices
    );
}

#[allow(unused)]
fn get_tile_at_cursor(
    cursor_coords: Res<MyWorldCoords>,
    mut tile_coords: ResMut<MyTileCoords>,
    map: Query<&Tilemap>,
) {
    let Ok(map) = map.get_single() else {
        return;
    };
    let tile = map.world_to_tile_coords(&cursor_coords.0);
    tile_coords.0 = tile;
    // eprintln!("tile_coords: {}/{}", tile_coords.0.x, tile_coords.0.y);
}
