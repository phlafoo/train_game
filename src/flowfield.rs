use std::{
    collections::BinaryHeap,
    f32::consts::{FRAC_1_SQRT_2, PI, TAU},
};

use bevy::{
    color::palettes::css::{BLACK, FUCHSIA, GREEN, MAROON, NAVY, OLIVE, SILVER, TEAL},
    math::vec2,
    prelude::*,
};
use bevy_rapier2d::dynamics::ExternalForce;

use crate::{
    camera::MainCamera,
    chaser::Chaser,
    config::{Config, DebugViews},
    cursor::MyWorldCoords,
    player::{self, Player},
    tilemap::*,
};

#[derive(Default)]
pub struct FlowfieldPlugin;

impl Plugin for FlowfieldPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().spawn(Flowfield::default());
        app.add_systems(
            Update,
            (
                update_target,
                update_cost,
                // update_flowfield,
                apply_force,
                draw_flowfield,
            )
                .chain(),
        )
        .add_systems(Update, print_cost_at_cursor)
        .add_systems(
            PostStartup,
            setup_flowfield
                .run_if(any_with_component::<Tilemap>)
                .after(player::spawn_player),
        )
        .register_type::<Flowfield>();
    }
}

pub const N_BITMASK: u8 = 0b0000_1000;
pub const E_BITMASK: u8 = 0b0000_0100;
pub const S_BITMASK: u8 = 0b0000_0010;
pub const W_BITMASK: u8 = 0b0000_0001;
pub const NE_BITMASK: u8 = 0b1100_0000;
pub const SE_BITMASK: u8 = 0b0110_0000;
pub const SW_BITMASK: u8 = 0b0011_0000;
pub const NW_BITMASK: u8 = 0b1001_0000;

// Each tile on the wall layer it turned into a bitmask
// Empty tiles are 0, wall tiles are 0b1111_1111, and subtiles get a bespoke bitmask that depends on
// the collision mesh. The bitmask defines which directions are blocked off when moving *from* the tile.
pub const WALL_BITMASK: u8 = u8::MAX;
pub const N_SUBTILE_BITMASK: u8 = N_BITMASK | NE_BITMASK | NW_BITMASK;
pub const E_SUBTILE_BITMASK: u8 = E_BITMASK | NE_BITMASK | SE_BITMASK;
pub const S_SUBTILE_BITMASK: u8 = S_BITMASK | SE_BITMASK | SW_BITMASK;
pub const W_SUBTILE_BITMASK: u8 = W_BITMASK | SW_BITMASK | NW_BITMASK;
pub const NE_SUBTILE_BITMASK: u8 = NE_BITMASK | N_BITMASK | E_BITMASK;
pub const SE_SUBTILE_BITMASK: u8 = SE_BITMASK | S_BITMASK | E_BITMASK;
pub const SW_SUBTILE_BITMASK: u8 = SW_BITMASK | S_BITMASK | W_BITMASK;
pub const NW_SUBTILE_BITMASK: u8 = NW_BITMASK | N_BITMASK | W_BITMASK;

#[derive(Component, Default, Clone, Reflect)]
pub struct Flowfield {
    /// Real time position of player.
    target: Vec2,
    /// Position of player for the current cost calculation. Gets updated right after entire cost
    /// grid is finished updating.
    transient_target: IVec2,
    /// Used to determine if cost grid should be recalculated.
    target_changed: bool,
    /// Stores cost from Dijkstra's algorithm and bool for if the tile has been visited.
    cost_grid: Vec<(u32, bool)>,
    /// Stores direction at each grid position.
    field: Vec<Option<Dir2>>,
    /// Used for calculating cost via Dijkstra's algorithm.
    heap: BinaryHeap<Node>,
    /// width in number of tiles
    pub width: isize,
    /// height in number of tiles
    pub height: isize,
}

impl Flowfield {
    const NEIGHBORS: [((isize, isize), Dir2, u8); 8] = [
        ((0, 1), Dir2::NORTH, N_BITMASK),         // N
        ((1, 0), Dir2::EAST, E_BITMASK),          // E
        ((0, -1), Dir2::SOUTH, S_BITMASK),        // S
        ((-1, 0), Dir2::WEST, W_BITMASK),         // W
        ((1, 1), Dir2::NORTH_EAST, NE_BITMASK),   // NE
        ((1, -1), Dir2::SOUTH_EAST, SE_BITMASK),  // SE
        ((-1, -1), Dir2::SOUTH_WEST, SW_BITMASK), // SW
        ((-1, 1), Dir2::NORTH_WEST, NW_BITMASK),  // NW
    ];

    // TODO cleanup
    #[inline(always)]
    pub fn get_flow_at_tile(
        &mut self,
        tile: Vec2,
        storage: &TileStorage,
        config: &Res<Config>,
    ) -> Dir2 {
        let i = (tile.x as isize + tile.y as isize * self.width) as usize;

        let cost = self.cost_grid[i].0;
        let line_of_sight = cost == self.get_minimum_cost_at_tile(tile);
        // let line_of_sight = cost == self.get_minimum_cost_at_index(i);

        if cost > config.flow_cost_threshold || !line_of_sight {
            if let Some(f) = self.field[i] {
                return f;
            }
        }

        let col = i as isize % self.width;
        let row = i as isize / self.width;
        let mut neighbor_wall_mask = 0_u8;

        // if not smoothed or target has changed
        if
        // !line_of_sight
        //     ||
        self.field[i].is_none()
            || self.target_changed
            || [0., 1., FRAC_1_SQRT_2].contains(&self.field[i].unwrap().x.abs())
        {
            // count_neighbor_check += 1;
            // if debug {
            //     println!("    checking neighbors...");
            // }
            let subtile_mask = storage.0[i];

            let mut min = u32::MAX;
            // let mut wall_adjacent = false;
            // let mut neighbors_wall = [false; 4];
            let mut delta = (0, 0);

            for &((dx, dy), dir, mask) in Self::NEIGHBORS.iter() {
                let next_col = col + dx;
                let next_row = row + dy;
                if next_col < 0
                    || next_col >= self.width
                    || next_row < 0
                    || next_row >= self.height
                    || subtile_mask & mask == mask
                {
                    continue;
                }
                let n = (next_col + next_row * self.width) as usize;
                // TODO use tile storage for wall check, can maybe check for subtiles also
                // 0110_0110
                // 0000_0010

                // 1001_1001
                //

                // 0.9238795, 0.38268343

                // maybe try    neighbor_wall_mask |= mask * (storage.tiles[n] != 0) as u8;
                //
                // if storage.tiles[n] != 0 {
                let neighbor_cost = self.cost_grid[n].0;
                // neighbor_wall_mask |= mask * (neighbor_cost == u32::MAX) as u8;
                if neighbor_cost == u32::MAX {
                    // wall_adjacent = true;
                    neighbor_wall_mask |= mask;
                    continue;
                }

                if neighbor_cost < min {
                    if (neighbor_wall_mask << 4) & mask != 0 {
                        // if (neighbor_wall_mask << 4) != 0 {
                        continue;
                    }
                    min = neighbor_cost;
                    self.field[i] = Some(dir);
                    delta = (dx, dy);
                }
            }
            if config.sub_diagonal {
                if let Some(dir) = &mut self.field[i] {
                    let is_diagonal = dir.x != 0. && dir.y != 0.;
                    if is_diagonal {
                        let n1 = i as isize + delta.0 * 2 + delta.1 * self.width;
                        let n2 = i as isize + delta.0 + 2 * delta.1 * self.width;
                        let c1 = self.cost_grid[n1 as usize].0;
                        let c2 = self.cost_grid[n2 as usize].0;
                        *dir = match c1.cmp(&c2) {
                            std::cmp::Ordering::Less => {
                                Dir2::new(dir.as_vec2() + vec2(delta.0 as f32, 0.)).unwrap()
                            }
                            std::cmp::Ordering::Greater => {
                                Dir2::new(dir.as_vec2() + vec2(0., delta.1 as f32)).unwrap()
                            }
                            std::cmp::Ordering::Equal => *dir,
                        };
                        // if c1 < c2 {
                        //     *dir = Dir2::new(dir.as_vec2() + vec2(delta.0 as f32, 0.)).unwrap();
                        //     // println!("dir: {:?}", dir);
                        // } else if c2 < c1 {
                        //     *dir = Dir2::new(dir.as_vec2() + vec2(0., delta.1 as f32)).unwrap();
                        // }
                    }
                }
            }
        }

        /*
        For tiles that have a direct line of sight to the target we set the flow to point directly
        at the target. To check for direct line of sight we compare actual cost the the theoretical
        cost assuming no obstacles between the tile and target.
        For tiles not lying directly on a cardinal/ordinal direction relative to the target this
        method can be optimistic which leads to smoothing in cases where there actually is an
        obstruction.
        To mitigate this I only smooth the flow if:
         - there are no adjacent walls, and
         - the pre-smoothed direction is not cardinal (except for tiles that are directly cardinal from target)
         */
        // if debug {
        //     println!(" neighbor_wall_mask == 0 == {}", neighbor_wall_mask == 0);
        //     println!(
        //         " cost == flowfield.get_minimum_cost_at_index(i) == {}",
        //         cost == flowfield.get_minimum_cost_at_index(i)
        //     );
        //     println!(
        //         " (flowfield.field[i].x != 0. && flowfield.field[i].y != 0.) == {}",
        //         (flowfield.field[i].x != 0. && flowfield.field[i].y != 0.)
        //     );
        //     println!(
        //         " (col as i32 == flowfield.transient_target.x || row as i32 == flowfield.transient_target.y) == {}",
        //         (col as i32 == flowfield.transient_target.x || row as i32 == flowfield.transient_target.y)
        //     );
        // }
        let is_diagonal = self.field[i].map_or(false, |f| f.x != 0. && f.y != 0.);
        let is_cardinal_to_target = !config.filter_cardinal
            || (col as i32 == self.transient_target.x || row as i32 == self.transient_target.y);
        let wall_adjacent = !config.filter_wall_adjacent || neighbor_wall_mask == 0;

        if wall_adjacent
            && line_of_sight
            && (is_diagonal || is_cardinal_to_target)
            && config.flowfield_smooth
        {
            // count_smoothed += 1;
            // if debug {
            //     println!("  smoothing...");
            // }
            // no obstacles, set flow to point directly at target
            let tile = get_tile_coords(i, self.width);
            // let dir_x = self.target.x - tile.0 as f32 - 0.5;
            // let dir_y = self.target.y - tile.1 as f32 - 0.5;
            let (dir_x, dir_y) = if cost < config.flow_cost_threshold {
                (
                    self.target.x - tile.0 as f32 - 0.5,
                    self.target.y - tile.1 as f32 - 0.5,
                )
            } else {
                (
                    self.transient_target.x as f32 - tile.0 as f32 - 0.5,
                    self.transient_target.y as f32 - tile.1 as f32 - 0.5,
                )
            };

            let dir = Dir2::from_xy(dir_x, dir_y).expect("Failed to make Dir2 from xy");
            self.field[i] = Some(dir);
        }
        self.field[i].unwrap_or(Dir2::Y)
    }

    pub fn get_index_at_tile(&self, tile: Vec2) -> Option<usize> {
        if tile.x.is_sign_negative()
            || tile.y.is_sign_negative()
            || tile.y as isize >= self.height
            || tile.x as isize >= self.width
        {
            return None;
        }
        Some(self.get_index_at_tile_unchecked(tile))
    }

    pub fn get_index_at_tile_unchecked(&self, tile: Vec2) -> usize {
        (tile.x as isize + tile.y as isize * self.width) as usize
    }

    pub fn get_minimum_cost_at_tile(&self, tile: Vec2) -> u32 {
        let x_diff = (self.transient_target.x - tile.x as i32).unsigned_abs();
        let y_diff = (self.transient_target.y - tile.y as i32).unsigned_abs();
        let diag_count = x_diff.min(y_diff);
        let straight_count = x_diff.max(y_diff) - diag_count;
        diag_count * 14 + straight_count * 10
    }

    #[allow(unused)]
    pub fn get_minimum_cost_at_index(&self, index: usize) -> u32 {
        let (x, y) = get_tile_coords(index, self.width);
        let x_diff = (self.transient_target.x - x as i32).unsigned_abs();
        let y_diff = (self.transient_target.y - y as i32).unsigned_abs();
        let diag_count = x_diff.min(y_diff);
        let straight_count = x_diff.max(y_diff) - diag_count;
        diag_count * 14 + straight_count * 10
    }
}
pub fn get_tile_coords(index: usize, width: isize) -> (u32, u32) {
    let x = index % width as usize;
    let y = index / width as usize;
    // TilePos::new(x, y)
    (x as u32, y as u32)
}

fn setup_flowfield(q_tilemap: Query<&Tilemap>, mut q_flowfield: Query<&mut Flowfield>) {
    let mut flowfield = q_flowfield.single_mut();

    let map = q_tilemap.single();

    flowfield.width = map.width as isize;
    flowfield.height = map.height as isize;
    flowfield.field = vec![None; map.width * map.height];

    flowfield.cost_grid = vec![(u32::MAX, false); map.width * map.height];
    flowfield.target_changed = true;
    flowfield.transient_target = IVec2::ZERO; // q_spawn.single().1.translation().xy().as_ivec2();

    info!("setup flowfield!");
}

fn update_target(
    q_player_transform: Query<&Transform, With<Player>>,
    mut q_flowfield: Query<&mut Flowfield>,
    q_map: Query<&Tilemap>,
) {
    let Ok(player_translation) = q_player_transform.get_single().map(|p| p.translation) else {
        return;
    };
    let Ok(map) = q_map.get_single() else {
        return;
    };
    let mut flowfield = q_flowfield.single_mut();

    flowfield.target = map.world_to_tile_coords(&player_translation.xy());

    if flowfield.target.as_ivec2() != flowfield.transient_target {
        flowfield.target_changed = true;
    }
}

/// Used for Dijkstra's
#[derive(PartialEq, Eq, Clone, Reflect)]
struct Node {
    index: usize,
    cost: u32,
}

impl Node {
    pub fn new(index: usize, cost: u32) -> Node {
        Node { index, cost }
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(other.cost.cmp(&self.cost))
    }
}

impl Ord for Node {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.cost.cmp(&self.cost)
    }
}

fn update_cost(
    time: Res<Time>,
    config: Res<Config>,
    mut q_flowfield: Query<&mut Flowfield>,
    q_tile_storage: Query<&TileStorage>,
) {
    let mut flowfield = q_flowfield.single_mut();

    // Cost grid has not been setup yet
    if flowfield.cost_grid.is_empty() {
        return;
    }
    // let target_changed = !q_target.is_empty();
    let storage = q_tile_storage.single();

    // It is too expensive to do Dijkstra's for the entire grid every frame so instead we amortize
    // the calculation across many frames. `config.seconds_per_iter` determines how long it should
    // take to calculate cost for the entire grid. This is combined with the frametime of the previous
    // frame to calculate `iter_per_update`, which determines how many steps of the algorithm we will
    // perform this frame.
    let tile_count = flowfield.width * flowfield.height;
    let fps = 1.0 / time.delta_seconds();
    let iter_per_update = (tile_count as f32 / (fps * config.seconds_per_iter)).max(1.0) as usize;

    if !flowfield.target_changed && flowfield.heap.is_empty() {
        // Player has not moved since the cost grid was fully calculated.
        return;
    }

    // If the heap is empty, we must initiate Dijkstra's from the player position
    if flowfield.heap.is_empty() {
        flowfield.transient_target = flowfield.target.as_ivec2();
        let Some(start_index) = flowfield.get_index_at_tile(flowfield.target) else {
            return;
        };
        // Mark all tiles not visited
        for (_, visited) in flowfield.cost_grid.iter_mut() {
            *visited = false;
        }
        // Init cost grid and heap at player position
        flowfield.cost_grid[start_index] = (0_u32, true);
        flowfield.heap.push(Node::new(start_index, 0_u32));
    }
    let width = flowfield.width;
    let height = flowfield.height;

    /// The order here is important. If there are walls north and/or east, then we don't want to calculate
    /// cost for the northeast tile. So we need to check all the cardinal directions before the ordinal ones.
    ///
    /// ((dx, dy), step_cost, wall_mask)
    const NEIGHBORS: [((isize, isize), u32, u8); 8] = [
        ((0, 1), 10, N_BITMASK),    // N
        ((1, 0), 10, E_BITMASK),    // E
        ((0, -1), 10, S_BITMASK),   // S
        ((-1, 0), 10, W_BITMASK),   // W
        ((1, 1), 14, NE_BITMASK),   // NE
        ((1, -1), 14, SE_BITMASK),  // SE
        ((-1, -1), 14, SW_BITMASK), // SW
        ((-1, 1), 14, NW_BITMASK),  // NW
    ];

    for _ in 0..iter_per_update {
        let Some(Node { index: i, cost }) = flowfield.heap.pop() else {
            break;
        };
        // Prev flow is invalid since we are recalculating the cost
        flowfield.field[i] = None;

        // Get coords of reference tile
        let x = i as isize % width;
        let y = i as isize / width;
        let mut wall_mask = 0;

        // Calculate cost for 8 neighbors
        for &((dx, dy), step_cost, mask) in NEIGHBORS.iter() {
            let next_x = x + dx;
            let next_y = y + dy;

            // TODO use index only (instead of x/y offset) by adding 1 tile thick padding around grid?
            // Bounds check
            if next_x < 0 || next_x >= width || next_y < 0 || next_y >= height {
                continue;
            }
            // Get index
            let n = (next_x + next_y * width) as usize;

            // If we hit a wall we need to update the wall mask even if this neighbor has been
            // visited from another reference tile
            if storage.0[n] != 0 {
                // => hit wall
                // The wall mask indicates which neighbors are walls
                wall_mask |= mask;
                flowfield.cost_grid[n].0 = u32::MAX; // Wall cost
                flowfield.cost_grid[n].1 = true; // Mark visited

                // Wall tiles will not be added to the heap, so the flow gets reset here.
                flowfield.field[n] = None;
                continue;
            }
            // If already visited, skip
            if flowfield.cost_grid[n].1 {
                continue;
            }
            // Since we check cardinal directions (step cost 10) before ordinal (diagonal) ones,
            // if are are checking a diagonal neighbor, we already know if there are any adjacent
            // walls (cardinally) blocking the path. Even with only 1 wall cardinally we don't want
            // to move diagonally along that direction (this helps chasers not smack into outside corners).
            // Example with north and east tiles w/ wall and northeast tile empty:
            //   After checking N, E, S, W neighbors, our wall_mask is the combination of N and E masks:
            //      0000_1000 | 0000_0100 = 0000_1100
            //   The mask when checking the NE neighbor is 1100_0000 so our condition evaluates like this:
            //      (0000_1100 << 4) & 1100_0000 = 1100_0000 != 0
            //   Then we know we can skip this diagonal.
            if step_cost == 14 && (wall_mask << 4) & mask != 0 {
                continue;
            }

            // Update cost and mark visited
            let new_cost = cost + step_cost;
            flowfield.cost_grid[n].0 = new_cost;
            flowfield.cost_grid[n].1 = true;

            flowfield.heap.push(Node::new(n, new_cost));
        }
    }

    // Stops recalculating cost when player stops moving
    if flowfield.heap.is_empty() {
        flowfield.target_changed = false;
    }
}

pub fn apply_force(
    mut q_flowfield: Query<&mut Flowfield>,
    mut q_map: Query<&Tilemap>,
    mut q_chasers: Query<(&mut ExternalForce, &mut Transform), With<Chaser>>,
    q_tile_storage: Query<&TileStorage>,
    time: Res<Time>,
    config: Res<Config>,
) {
    let Ok(map) = q_map.get_single_mut() else {
        return;
    };
    let mut flowfield = q_flowfield.single_mut();
    let storage = q_tile_storage.single();

    for (mut force, mut transform) in q_chasers.iter_mut() {
        let translation = transform.translation;
        // let world_coords = tile_to_world_coords((tile_x, tile_y), &map_info, &translation);
        let tile_coords = map.world_to_tile_coords(&translation.xy());

        let new_dir = flowfield.get_flow_at_tile(tile_coords, storage, &config);

        force.force = new_dir * config.flowfield_force;

        // Update rotation to face the direction of travel
        let mut new_angle = new_dir.to_angle();
        if new_angle < 0.0 {
            new_angle += TAU;
        }
        let new_angle = Rot2::radians(new_angle).as_radians();
        let target = Quat::from_rotation_z(new_angle - 0.75 * PI);
        let s = 6.0 * time.delta_seconds();
        transform.rotation = transform.rotation.lerp(target, s);
    }
}

// TODO rework
fn draw_flowfield(
    q_ortho: Query<(&OrthographicProjection, &GlobalTransform), With<MainCamera>>,
    mut gizmos: Gizmos,
    mut q_flowfield: Query<&mut Flowfield>,
    mut q_map: Query<&Tilemap>,
    q_tile_storage: Query<&TileStorage>,
    debug_views: Res<DebugViews>,
    config: Res<Config>,
) {
    if !debug_views.render_flowfield {
        return;
    }
    let Ok(map) = q_map.get_single_mut() else {
        return;
    };

    let (ortho, transform) = q_ortho.single();

    let mut flowfield = q_flowfield.single_mut();
    let width = flowfield.width;
    let target = flowfield.target;
    let world_coords = map
        .tile_to_world_coords((target.x as u32, target.y as u32))
        .as_vec2();
    gizmos.rect_2d(
        world_coords,
        0.0,
        vec2(map.tile_width - 2.0, map.tile_height - 2.0),
        MAROON,
    );
    let storage = q_tile_storage.single();

    let len = (width * flowfield.height) as usize;

    for i in 0..len {
        let tile = get_tile_coords(i, width);

        let dir = if debug_views.compute_full_flow {
            flowfield.get_flow_at_tile(vec2(tile.0 as f32, tile.1 as f32), storage, &config)
        } else {
            let Some(dir) = flowfield.field[i] else {
                continue;
            };
            dir
        };
        let (tile_x, tile_y) = get_tile_coords(i, width);
        let world_coords = map.tile_to_world_coords((tile_x, tile_y)).as_vec2();

        if ortho
            .area
            .contains(world_coords - transform.translation().xy())
        {
            let cost = flowfield.cost_grid[i].0;
            const SIN_5_FRAC_PI_8: f32 = 0.9238795;
            const SIN_FRAC_PI_8: f32 = 0.38268343;

            // [0., 1., FRAC_1_SQRT_2].contains(&flowfield.field[i].x.abs())
            let color = if [0., 1., FRAC_1_SQRT_2].contains(&dir.x.abs()) {
                BLACK.with_alpha(0.8)
            } else if [SIN_FRAC_PI_8, SIN_5_FRAC_PI_8].contains(&dir.x.abs()) {
                Srgba::rgb(0.3, 0.4, 0.0).with_alpha(0.99)
            } else if !flowfield.cost_grid[i].1 || cost > config.flow_cost_threshold {
                TEAL.with_alpha(0.9)
            } else {
                MAROON.with_alpha(0.8)
            };

            gizmos
                .arrow_2d(world_coords, world_coords + dir.as_vec2() * 10.0, color)
                .with_tip_length(3.0);
        }
    }
}

fn print_cost_at_cursor(
    buttons: Res<ButtonInput<MouseButton>>,
    cursor: Res<MyWorldCoords>,
    mut q_flowfield: Query<&mut Flowfield>,
    mut q_map: Query<&Tilemap>,
) {
    if !buttons.just_pressed(MouseButton::Left) {
        return;
    }

    let Ok(map) = q_map.get_single_mut() else {
        return;
    };

    let flowfield = q_flowfield.single_mut();

    let tile_coords = map.world_to_tile_coords(&cursor.0);
    let Some(index) = flowfield.get_index_at_tile(tile_coords) else {
        return;
    };
    println!(
        "world: ({}, {}),  tile: ({}, {}),  cost: {},  visited? {}",
        cursor.0.x,
        cursor.0.y,
        tile_coords.x,
        tile_coords.y,
        flowfield.cost_grid[index].0,
        flowfield.cost_grid[index].1
    );
}
