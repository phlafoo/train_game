use bevy::{input::common_conditions::input_toggle_active, prelude::*, window::PrimaryWindow};
use bevy_egui::egui::{self, CollapsingHeader, RichText};
use bevy_inspector_egui::{bevy_egui::EguiContext, bevy_inspector::ui_for_resource, prelude::*};

pub struct ConfigPlugin;

impl Plugin for ConfigPlugin {
    fn build(&self, app: &mut App) {
        app
            // .add_plugins(DefaultInspectorConfigPlugin)
            //     .add_plugins(bevy_egui::EguiPlugin)
            // .add_plugins(WorldInspectorPlugin::default())
            .init_resource::<Config>()
            .register_type::<Config>()
            .init_resource::<DebugViews>()
            .register_type::<DebugViews>()
            // .add_plugins(
            //     ResourceInspectorPlugin::<Config>::default()
            //         .run_if(input_toggle_active(false, KeyCode::KeyC)),
            // )
            .add_systems(
                Update,
                ui_config.run_if(input_toggle_active(false, KeyCode::KeyC)),
            );
    }
}

#[derive(Resource, Reflect, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct DebugViews {
    pub render_objects: bool,
    pub render_flowfield: bool,
    pub compute_full_flow: bool,
    pub render_movement: bool,
    pub render_rapier: bool,
    pub perf_overlay: bool,
    pub perf_extras: bool,
}

impl Default for DebugViews {
    fn default() -> Self {
        Self {
            render_objects: false,
            render_flowfield: false,
            compute_full_flow: false,
            render_movement: false,
            render_rapier: false,
            perf_overlay: true,
            perf_extras: false,
        }
    }
}

#[derive(Resource, Reflect, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct Config {
    /// Smooth out the flowfield at tiles that have a direct sight line to the target
    pub flowfield_smooth: bool,
    /// Chasers in this range can follow player per frame
    #[inspector(min = 0, max = 2000)]
    pub flow_cost_threshold: u32,
    /// How often the entire cost grid is calculated
    #[inspector(min = 0.01, max = 10.0)]
    pub seconds_per_iter: f32,

    pub max_chasers: usize,
    #[inspector(min = 0.0, max = 200.0)]
    pub chaser_detection_radius: f32,
    #[inspector(min = 0.0, max = 1_000_000_000.0)]
    pub chaser_avoidance_mul: f32,
    #[inspector(min = 0.0, max = 1_000_000_000.0)]
    pub chaser_avoidance_max: f32,
    #[inspector(min = 0.0, max = 10.0, speed = 0.01)]
    pub chaser_rng_force: f32,

    #[inspector(min = 0.0, max = 0.1, speed = 0.0001)]
    pub stick_deadzone: f32,
    /// How far the player can get from the center of the screen
    #[inspector(min = 2.0, max = 1500.0, speed = 1.0)]
    pub camera_follow_dist: f32,
    /// Set below 5.0 to turn off limiter
    #[inspector(min = 0.0, max = 400.0, speed = 1.0)]
    pub framerate: f64,
    pub temp: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            flowfield_smooth: true,
            flow_cost_threshold: 150,
            seconds_per_iter: 0.4,
            max_chasers: 5000,
            chaser_detection_radius: 35.0,
            chaser_avoidance_mul: 3_200_000.0,
            chaser_avoidance_max: 30_000.0,
            chaser_rng_force: 0.4,
            stick_deadzone: 0.07460,
            camera_follow_dist: 125.0,
            framerate: 0.0,
            temp: 100.0,
        }
    }
}

fn ui_config(world: &mut World) {
    let egui_context = world
        .query_filtered::<&mut EguiContext, With<PrimaryWindow>>()
        .get_single(world);

    let Ok(egui_context) = egui_context else {
        println!("Failed to get egui context!");
        return;
    };
    let mut egui_context = egui_context.clone();

    let title = RichText::new("Config").text_style(egui::TextStyle::Body);
    egui::Window::new(title).show(egui_context.get_mut(), |ui| {
        egui::ScrollArea::both().show(ui, |ui| {
            CollapsingHeader::new("Config")
                .default_open(true)
                .show(ui, |ui| ui_for_resource::<Config>(world, ui));
            CollapsingHeader::new("Debug Views")
                .default_open(true)
                .show(ui, |ui| ui_for_resource::<DebugViews>(world, ui));
        });
    });
}
