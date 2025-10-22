// top of file
use crate::app::AppState;
use crate::core::galaxy_camera::{GalaxyCamera, GalaxyCameraPlugin, MainCamera};
use crate::core::planet_debug::PlanetDebugPlugin;
use crate::core::skybox::{Skybox, SkyboxPlugin, StarfieldAssets};
use crate::game::world::planet::{
    spawn_random_planet_inner,
    sync_planet_material_debug,
    toggle_planet_wireframe,
    update_planet_lod,
    PlanetDebugConfig,
    PlanetParams,
    PlanetSettings,
    PlanetSurfaceMaterial,
    PlanetSurfaceParams, // <-- add
};
use crate::game::world::sampling::FlatSamplerRes;
<<<<<<< HEAD
use crate::game::world::terrain::MapSettings;
use bevy::log::info;
use bevy::math::primitives::Sphere;
use bevy::pbr::{wireframe::WireframePlugin, MaterialPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::renderer::RenderDevice; // <-- add
use bevy::render::{Render, RenderApp, RenderSystems};

pub mod ui;
pub mod world;
=======

pub mod world;
pub mod ui;
>>>>>>> 7e6f9f8ca734934589b0e887862f7c5feb852eed

#[derive(Component)]
pub struct InGameRoot;

#[derive(Component)]
struct SunLight {
    base_pitch: f32,
    base_yaw: f32,
}

const SUN_BASE_PITCH: f32 = -0.9;
const SUN_BASE_YAW: f32 = 0.7;

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapSettings>()
            .init_resource::<PlanetParams>()
            .init_resource::<FlatSamplerRes>()
            .init_resource::<PlanetDebugConfig>()
            .add_plugins(MaterialPlugin::<PlanetSurfaceMaterial>::default())
            .add_plugins(WireframePlugin::default())
            .add_plugins(PlanetDebugPlugin)
            .add_plugins(GalaxyCameraPlugin)
            .add_plugins(SkyboxPlugin)
            .add_plugins(ui::dev_panel::DevPanelPlugin)
            .add_systems(
                Update,
                (
                    update_planet_lod,
                    sync_planet_material_debug,
                    toggle_planet_wireframe,
                    sync_sun_with_planet_rotation,
                )
                    .run_if(in_state(AppState::InGame)),
            )
            .add_systems(OnEnter(AppState::InGame), setup_world);

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                inspect_planet_material_layout.in_set(RenderSystems::Render),
            );
        }
    }
}

fn inspect_planet_material_layout(render_device: Res<RenderDevice>) {
    let entries =
        <PlanetSurfaceParams as AsBindGroup>::bind_group_layout_entries(&render_device, false)
            .into_iter()
            .map(|e| (e.binding, e.visibility, e.ty))
            .collect::<Vec<_>>();
    info!("PlanetSurfaceMaterial bindings: {:?}", entries);
}

fn setup_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut planet_materials: ResMut<Assets<PlanetSurfaceMaterial>>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    mut sampler_res: ResMut<FlatSamplerRes>,
    map: Res<MapSettings>,
    params: Res<PlanetParams>,
    settings: Res<PlanetSettings>,
    debug: Res<PlanetDebugConfig>,
    starfield: Res<StarfieldAssets>,
    existing_camera: Query<Entity, With<MainCamera>>,
    mut existing_light: Query<(Entity, &mut Transform, Option<&SunLight>), With<DirectionalLight>>,
    existing_skybox: Query<(), With<Skybox>>,
) {
    
    sampler_res.0.seed = map.seed;

    spawn_random_planet_inner(
        &mut commands,
        &mut meshes,
        &mut *planet_materials,
        &mut *standard_materials,
        &*sampler_res,
        &map,
        &params,
        &settings,
        &debug,
    );
    info!("spawned planet with radius {}", params.radius);

    let target_rotation = sun_rotation_from_params(params.rotation_deg);
    if let Some((entity, mut transform, has_component)) = existing_light.iter_mut().next() {
        transform.rotation = target_rotation;
        if has_component.is_none() {
            commands.entity(entity).insert(SunLight {
                base_pitch: SUN_BASE_PITCH,
                base_yaw: SUN_BASE_YAW,
            });
        }
    } else {
        commands.spawn((
            DirectionalLight {
                shadows_enabled: true,
                illuminance: 25_000.0,
                ..default()
            },
            Transform::from_rotation(target_rotation),
            SunLight {
                base_pitch: SUN_BASE_PITCH,
                base_yaw: SUN_BASE_YAW,
            },
            Name::new("Sun"),
        ));
        info!("spawned directional light");
    }

    let camera_entity = if let Some(entity) = existing_camera.iter().next() {
        entity
    } else {
        let (camera_state, initial_transform) = GalaxyCamera::new(params.radius);
        let entity = commands
            .spawn((
                Camera3d::default(),
                initial_transform,
                camera_state,
                MainCamera,
                Name::new("GameCamera"),
            ))
            .id();
        info!("spawned main camera");
        entity
    };

    if existing_skybox.is_empty() {
        let sky_scale = params.radius.max(1.0) * 400.0;
        let skybox_entity = commands
            .spawn((
                Skybox,
                Mesh3d(starfield.mesh.clone()),
                MeshMaterial3d(starfield.material.clone()),
                Transform::from_scale(Vec3::splat(sky_scale)),
                Visibility::Visible,
                Name::new("Skybox"),
            ))
            .id();
        commands.entity(camera_entity).add_child(skybox_entity);
    }

    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(50.0))),
        MeshMaterial3d(standard_materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.2, 0.2),
            ..default()
        })),
        Transform::from_xyz(0.0, params.radius + 60.0, 0.0),
        Name::new("DebugSphere"),
    ));
}

fn sync_sun_with_planet_rotation(
    params: Res<PlanetParams>,
    mut lights: Query<(&SunLight, &mut Transform), With<DirectionalLight>>,
) {
    if !params.is_changed() {
        return;
    }

    for (sun, mut transform) in &mut lights {
        let yaw = sun.base_yaw + params.rotation_deg.to_radians();
        let rotation = Quat::from_euler(EulerRot::XYZ, sun.base_pitch, yaw, 0.0);
        transform.rotation = rotation;
    }
}

fn sun_rotation_from_params(rotation_deg: f32) -> Quat {
    let yaw = SUN_BASE_YAW + rotation_deg.to_radians();
    Quat::from_euler(EulerRot::XYZ, SUN_BASE_PITCH, yaw, 0.0)
}
