use bevy::prelude::*;
use bevy::math::primitives::Cuboid;
use crate::app::AppState;

pub mod world;
pub mod ui;

use crate::game::world::terrain_stream::{
    WantedPatches, compute_wanted_patches, apply_patch_streaming,
};
use crate::game::world::sampling::{FlatSamplerRes, FlatSampler};
use crate::game::world::patch::PatchGrid;

use world::terrain::MapSettings;

#[derive(Component)]
struct InGameRoot;

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        // resources
        app.init_resource::<MapSettings>();
        app.insert_resource(PatchGrid::default());
        app.insert_resource(FlatSamplerRes(FlatSampler {
            seed: 12345,
            base_freq: 0.0015,
            height_amp: 40.0,
        }));
        app.init_resource::<WantedPatches>();

        // dev panel
        app.add_plugins(ui::dev_panel::DevPanelPlugin);

        // lifecycle
        app.add_systems(OnEnter(AppState::InGame), setup_world);
        app.add_systems(OnExit(AppState::InGame), cleanup_world);

        // streaming
        app.add_systems(
            Update,
            (compute_wanted_patches, apply_patch_streaming)
                .run_if(in_state(AppState::InGame)),
        );
    }
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut map: ResMut<MapSettings>,
) {
    // small prop so the scene isn't empty
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mat  = materials.add(Color::srgb(0.2, 0.6, 0.9));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_xyz(0.0, 0.5, 3.5),
        InGameRoot,
        Name::new("DemoCube"),
    ));

    // new seed per run
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    map.seed = (nanos & 0xFFFF_FFFF_FFFF_FFFF) as u64;

    // NOTE: streaming patches will appear automatically
}

fn cleanup_world(
    mut commands: Commands,
    roots: Query<Entity, With<InGameRoot>>,
    children_q: Query<&Children>,
    cameras: Query<Entity, With<Camera>>,
) {
    for e in &roots {
        despawn_recursive(&mut commands, e, &children_q);
    }
    for c in &cameras {
        despawn_recursive(&mut commands, c, &children_q);
    }
}

// Works on 0.17 (no Commands::despawn_recursive)
fn despawn_recursive(commands: &mut Commands, entity: Entity, children_q: &Query<&Children>) {
    if let Ok(children) = children_q.get(entity) {
        // Each item from `children.iter()` is `Entity` already.
        for child in children.iter() {
            despawn_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
}
