use crate::app::AppState;

use crate::game::world::planet::{spawn_random_planet_inner, PlanetParams};
use crate::game::world::sampling::FlatSamplerRes;
use crate::game::world::terrain::MapSettings;
use bevy::prelude::*;
pub mod ui;
pub mod world;
use crate::game::world::planet::{auto_clip_planes, update_planet_lod};
#[derive(Component)]
struct InGameRoot;

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app
            // These three are REQUIRED before we enter InGame.
            .init_resource::<MapSettings>()
            .init_resource::<PlanetParams>()
            .init_resource::<FlatSamplerRes>()
            .add_systems(
                Update,
                (update_planet_lod, auto_clip_planes).run_if(in_state(AppState::InGame)),
            )
            // Your dev UI etc.
            .add_plugins(ui::dev_panel::DevPanelPlugin)
            // When we enter InGame, set up the world
            .add_systems(OnEnter(AppState::InGame), setup_world);
    }
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    sampler_res: Res<FlatSamplerRes>,
    map: Res<MapSettings>,
    params: Res<PlanetParams>,
) {
    // If you want this extra-safe during bring-up, you could use Option<Res<_>>.
    spawn_random_planet_inner(
        &mut commands,
        &mut meshes,
        &mut materials,
        &sampler_res,
        &map,
        &params,
    );
}
