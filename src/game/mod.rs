use bevy::prelude::*;
use bevy::math::primitives::Cuboid;
use crate::app::AppState;

pub mod world;
pub mod ui;

#[derive(Component)] struct InGameRoot;

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<world::terrain::MapSettings>()     // <-- add
           .add_plugins(ui::dev_panel::DevPanelPlugin)         // <-- add
           .add_systems(OnEnter(AppState::InGame), setup_world)
           .add_systems(OnExit(AppState::InGame), cleanup_world);
    }
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut map: ResMut<world::terrain::MapSettings>,  // <-- use the resource
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 40.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn(DirectionalLight::default());

    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mat = materials.add(Color::srgb(0.2, 0.6, 0.9));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_xyz(0.0, 0.5, 3.5),
        InGameRoot,
    ));

    // fresh seed per run
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    map.seed = (nanos & 0xFFFF_FFFF_FFFF_FFFF) as u64;

    // spawn map using the shared resource
    use world::terrain::{spawn_random_map};
    spawn_random_map(&mut commands, &mut meshes, &mut materials, &map);
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

fn despawn_recursive(commands: &mut Commands, entity: Entity, children_q: &Query<&Children>) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
}
