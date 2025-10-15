use bevy::asset::{AssetMetaCheck, AssetPlugin};
use bevy::prelude::*;
use crate::game::world::terrain::{
    MapSettings, MapRoot, spawn_random_map, reroll_system,
};
use crate::core::camera::EditorCameraPlugin;


#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
pub enum AppState {
    #[default]
    Menu,
    InGame,
}

pub fn build_app() -> App {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        meta_check: AssetMetaCheck::Never,
        ..default()
    }));

    app.init_state::<AppState>();

    app.add_plugins((
        crate::loading::LoadingPlugin,
        crate::menu::MenuPlugin,
        crate::game::GamePlugin,
    ));

    app.add_plugins(EditorCameraPlugin);

    // Terrain settings available globally
    app.insert_resource(MapSettings::default());

    // Create terrain on enter, clean up on exit
    app.add_systems(OnEnter(AppState::InGame), spawn_map_system);
    app.add_systems(OnExit(AppState::InGame), despawn_map_system);

    // Reroll on R while in-game (matches your current terrain.rs)
    app.add_systems(Update, reroll_system.run_if(in_state(AppState::InGame)));

    app
}

// ---- systems ----

fn spawn_map_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    settings: Res<MapSettings>,
) {
    spawn_random_map(&mut commands, &mut meshes, &mut materials, &settings);
}

fn despawn_map_system(
    mut commands: Commands,
    roots: Query<Entity, With<MapRoot>>,
) {
    // Bevy 0.17: .despawn() removes children too
    for e in &roots {
        commands.entity(e).despawn();
    }
}
