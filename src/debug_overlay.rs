use bevy::prelude::*;
use iyes_perf_ui::prelude::*;

use crate::config::DebugViews;

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        // we want Bevy to measure these values for us:
        app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin)
            .add_plugins(bevy::diagnostic::EntityCountDiagnosticsPlugin)
            .add_plugins(bevy::diagnostic::SystemInformationDiagnosticsPlugin)
            .add_plugins(PerfUiPlugin)
            .add_systems(Update, toggle.before(iyes_perf_ui::PerfUiSet::Setup));
    }
}

#[derive(Component)]
struct PerfExtras;

fn toggle(
    config: Res<DebugViews>,
    mut commands: Commands,
    q_root: Query<Entity, With<PerfUiRoot>>,
    q_root_extras: Query<&PerfExtras>,
) {
    if let Ok(e) = q_root.get_single() {
        let has_extras = q_root_extras.get(e).is_ok();
        if !config.perf_overlay
            || (config.perf_extras && !has_extras)
            || (!config.perf_extras && has_extras)
        {
            // despawn the existing Perf UI
            commands.entity(e).despawn_recursive();
        }
    } else if config.perf_overlay {
        let perf_bundle = (PerfUiEntryFPS::default(), PerfUiEntryFrameTime::default());
        let perf_extras = (
            PerfUiEntryFPSWorst::default(),
            PerfUiEntryFrameTimeWorst::default(),
            PerfUiEntryEntityCount::default(),
            PerfUiEntryCpuUsage::default(),
            PerfUiEntryMemUsage::default(),
            PerfUiEntryWindowResolution::default(),
            PerfUiEntryWindowScaleFactor::default(),
            PerfUiEntryWindowMode::default(),
            PerfUiEntryWindowPresentMode::default(),
            PerfExtras,
        );
        if config.perf_extras {
            commands.spawn((PerfUiRoot::default(), perf_bundle, perf_extras));
        } else {
            commands.spawn((PerfUiRoot::default(), perf_bundle));
        }
    }
}
