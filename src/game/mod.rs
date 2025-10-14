use bevy::prelude::*;
use bevy::math::primitives::Cuboid;
use crate::app::AppState;

#[derive(Component)] struct InGameRoot;

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), setup_world)
           .add_systems(OnExit(AppState::InGame), cleanup_world);
    }
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn(Camera3d::default());
    commands.spawn(DirectionalLight::default());

    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let mat = materials.add(Color::srgb(0.2, 0.6, 0.9));
    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(mat),
        Transform::from_xyz(0.0, 0.5, 3.5),
        InGameRoot,
    ));
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

