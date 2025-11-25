// top of file
use crate::app::AppState;
use crate::core::galaxy_camera::{GalaxyCamera, GalaxyCameraPlugin, MainCamera};
use crate::core::planet_debug::PlanetDebugPlugin;
use crate::core::surface_model::PlanetSurfaceModel;
use crate::core::skybox::{Skybox, SkyboxPlugin, StarfieldAssets};
<<<<<<< HEAD
use crate::game::commands::{
    debug_pick_surface_coord, init_surface_pick_res, LastSurfacePick,
};
use crate::game::planet_surface::virtual_texture::{PlanetPatchCache, PlanetPatchManifest};
use crate::game::units::{update_ground_anchors_system, update_unit_movement_system};
use crate::game::world::planet::{
    spawn_random_planet_inner, spin_planet_clouds, sync_planet_material_uniforms,
    toggle_planet_wireframe, update_planet_lod, AtmosphereMaterial, PlanetDebugConfig,
    PlanetParams, PlanetSettings, PlanetSurfaceMaterial, DEFAULT_BIOME_MAP_RESOLUTION,
=======
use crate::game::planet_surface::{
    asset_loader::PatchAssetState,
    manager::{update_planet_context, PlanetContext, PlanetLodConfig},
    procedural_loader::{
        climate_profiler_finish_frame, process_patch_queue, prune_surface_patches, ClimateProfiler,
    },
    render::{
        attach_prop_gizmos, update_patch_stats, PatchCacheMetrics, PatchMaterialLibrary,
        PatchRegistry, PatchStats, PropGizmoAssets,
    },
    stream::{drain_requests_system, PatchLoadTasks, PatchRequestQueue},
};
use crate::game::ui::pause_menu::pause_menu_hidden;
use crate::game::ui::settings_menu::settings_menu_hidden;
use crate::game::world::planet::{
    spawn_random_planet_inner, spin_planet_clouds, sync_orbit_shell_visibility,
    sync_planet_material_uniforms, toggle_planet_wireframe, update_planet_lod, AtmosphereMaterial,
    PlanetDebugConfig, PlanetEntity, PlanetParams, PlanetSettings, PlanetSurfaceMaterial,
    PlanetTag,
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
};
use crate::game::world::sampling::FlatSamplerRes;
use crate::game::world::local_patch::{update_local_surface_patch_system, LocalSurfacePatch};
use crate::game::world::surface_grid::SurfaceGrid;
use crate::game::world::terrain::MapSettings;
<<<<<<< HEAD
use bevy::asset::AssetServer;
use bevy::ecs::system::SystemParam;
use bevy::log::{info, warn};
=======
use bevy::ecs::schedule::IntoScheduleConfigs;
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
use bevy::{
    camera::visibility::NoFrustumCulling,
    math::{primitives::Sphere, EulerRot, Quat, Vec3, Vec4},
    pbr::{wireframe::WireframePlugin, MaterialPlugin, StandardMaterial},
    prelude::*,
    render::{
        render_resource::AsBindGroup, renderer::RenderDevice, Render, RenderApp, RenderSystems,
    },
};
<<<<<<< HEAD
use std::marker::PhantomData;

pub mod commands;
=======
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
pub mod planet_surface;
pub mod ui;
pub mod units;
pub mod world;

#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct InGameSystemSet;

#[derive(Component)]
pub struct InGameRoot;

#[derive(Component)]
struct SunLight {
    base_pitch: f32,
    base_yaw: f32,
}

const SUN_BASE_PITCH: f32 = -0.9;
const SUN_BASE_YAW: f32 = 0.7;
const SUN_LIGHT_ILLUMINANCE: f32 = 90_000.0;
const SUN_POINT_INTENSITY: f32 = 2.5e8;
const SUN_POINT_RANGE_FACTOR: f32 = 12.0;
const SUN_DISTANCE_FACTOR: f32 = 3.6;
const SUN_DISTANCE_MIN: f32 = 5_000.0;
const SUN_DISTANCE_MAX_FACTOR: f32 = 12.0;
const SUN_RADIUS_FACTOR: f32 = 0.28;
const SUN_RADIUS_MIN: f32 = 1_000.0;
const SUN_RADIUS_MAX_FACTOR: f32 = 4.0;

#[derive(Resource, Clone, Copy, Debug)]
pub struct SunSettings {
    pub brightness: f32,
}

impl Default for SunSettings {
    fn default() -> Self {
        Self { brightness: 0.10 }
    }
}

impl SunSettings {
    #[inline]
    fn directional_illuminance(self) -> f32 {
        SUN_LIGHT_ILLUMINANCE * self.brightness.max(0.0)
    }

    #[inline]
    fn point_intensity(self) -> f32 {
        SUN_POINT_INTENSITY * self.brightness.max(0.0)
    }
}

fn sun_visual_distance(radius: f32) -> f32 {
    let base = radius.max(1.0);
    let max_distance = (base * SUN_DISTANCE_MAX_FACTOR).max(SUN_DISTANCE_MIN);
    let min_distance = SUN_DISTANCE_MIN.min(max_distance);
    (base * SUN_DISTANCE_FACTOR).clamp(min_distance, max_distance)
}

fn sun_visual_radius(radius: f32) -> f32 {
    let base = radius.max(1.0);
    let max_radius = (base * SUN_RADIUS_MAX_FACTOR).max(SUN_RADIUS_MIN);
    let min_radius = SUN_RADIUS_MIN.min(max_radius);
    (base * SUN_RADIUS_FACTOR).clamp(min_radius, max_radius)
}

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapSettings>()
            .init_resource::<PlanetParams>()
            .init_resource::<FlatSamplerRes>()
            .init_resource::<PlanetDebugConfig>()
            .init_resource::<SunSettings>()
            .init_resource::<SunDirection>()
<<<<<<< HEAD
            .init_resource::<PlanetSurfaceModel>()
            .init_resource::<SurfaceGrid>()
            .init_resource::<LocalSurfacePatch>()
            .init_resource::<LastSurfacePick>()
=======
            .init_resource::<PlanetEntity>()
            .init_resource::<PlanetContext>()
            .init_resource::<PlanetLodConfig>()
            .init_resource::<PatchRequestQueue>()
            .init_resource::<PatchLoadTasks>()
            .init_resource::<PatchAssetState>()
            .init_resource::<PatchRegistry>()
            .init_resource::<PatchMaterialLibrary>()
            .init_resource::<PropGizmoAssets>()
            .init_resource::<ClimateProfiler>()
            .init_resource::<PatchStats>()
            .init_resource::<PatchCacheMetrics>()
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
            .add_plugins(MaterialPlugin::<PlanetSurfaceMaterial>::default())
            .add_plugins(MaterialPlugin::<AtmosphereMaterial>::default())
            .add_plugins(WireframePlugin::default())
            .add_plugins(PlanetDebugPlugin)
            .add_plugins(GalaxyCameraPlugin)
            .add_plugins(SkyboxPlugin)
            .add_plugins(ui::dev_panel::DevPanelPlugin)
<<<<<<< HEAD
            .configure_sets(
                Update,
                InGameSystemSet.run_if(in_state(AppState::InGame)),
            )
            .add_systems(
                Update,
                update_sun_direction_from_transform.in_set(InGameSystemSet),
            )
            .add_systems(
                Update,
                (
                    update_ground_anchors_system,
                    update_unit_movement_system,
=======
            .add_plugins(ui::pause_menu::PauseMenuPlugin)
            .add_plugins(ui::settings_menu::SettingsMenuPlugin)
            .add_systems(
                Update,
                (
                    update_sun_direction_from_transform,
                    update_planet_lod,
                    update_planet_context,
                    sync_orbit_shell_visibility,
                    drain_requests_system,
                    process_patch_queue,
                    attach_prop_gizmos,
                    climate_profiler_finish_frame,
                    prune_surface_patches,
                    sync_planet_material_uniforms,
                    toggle_planet_wireframe,
                    sync_sun_with_planet_rotation,
                    apply_sun_settings,
                    enforce_sun_visibility,
                    spin_planet_clouds,
                    update_patch_stats,
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
                )
                    .run_if(in_state(AppState::InGame))
                    .run_if(pause_menu_hidden)
                    .run_if(settings_menu_hidden),
            )
<<<<<<< HEAD
            .add_systems(Update, update_planet_lod.in_set(InGameSystemSet))
            .add_systems(
                Update,
                sync_planet_material_uniforms.in_set(InGameSystemSet),
            )
            .add_systems(Update, toggle_planet_wireframe.in_set(InGameSystemSet))
            .add_systems(
                Update,
                sync_sun_with_planet_rotation.in_set(InGameSystemSet),
            )
            .add_systems(Update, apply_sun_settings.in_set(InGameSystemSet))
            .add_systems(
                Update,
                enforce_sun_visibility.in_set(InGameSystemSet),
            )
            .add_systems(Update, spin_planet_clouds.in_set(InGameSystemSet))
            .add_systems(
                Update,
                debug_pick_surface_coord.in_set(InGameSystemSet),
            )
            .add_systems(
                Update,
                update_local_surface_patch_system.run_if(in_state(AppState::InGame)),
            )
            .add_systems(Startup, init_surface_pick_res)
            .add_systems(OnEnter(AppState::InGame), setup_world)
            .add_systems(
                OnEnter(AppState::InGame),
                load_planet_patch_manifest,
            );
=======
            .add_systems(OnEnter(AppState::InGame), setup_world)
            .add_systems(OnExit(AppState::InGame), cleanup_ingame_world);
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                inspect_planet_material_layout.in_set(RenderSystems::Render),
            );
        }
    }
}

fn inspect_planet_material_layout(render_device: Res<RenderDevice>, mut logged: Local<bool>) {
    if *logged {
        return;
    }
    *logged = true;

    let base_entries =
        <StandardMaterial as AsBindGroup>::bind_group_layout_entries(&render_device, false)
            .into_iter()
            .map(|e| (e.binding, e.visibility, e.ty))
            .collect::<Vec<_>>();
    let extended_entries =
        <PlanetSurfaceMaterial as AsBindGroup>::bind_group_layout_entries(&render_device, false)
            .into_iter()
            .map(|e| (e.binding, e.visibility, e.ty))
            .collect::<Vec<_>>();
    debug!("StandardMaterial bindings: {:?}", base_entries);
    debug!("PlanetSurfaceMaterial bindings: {:?}", extended_entries);
}

#[derive(SystemParam)]
struct SetupWorldAssets<'w, 's> {
    meshes: ResMut<'w, Assets<Mesh>>,
    planet_materials: ResMut<'w, Assets<PlanetSurfaceMaterial>>,
    atmosphere_materials: ResMut<'w, Assets<AtmosphereMaterial>>,
    standard_materials: ResMut<'w, Assets<StandardMaterial>>,
    images: ResMut<'w, Assets<Image>>,
    #[allow(dead_code)]
    _marker: PhantomData<&'s ()>,
}

fn setup_world(
    mut commands: Commands,
    mut assets: SetupWorldAssets,
    mut sampler_res: ResMut<FlatSamplerRes>,
    map: Res<MapSettings>,
    params: Res<PlanetParams>,
    settings: Res<PlanetSettings>,
    debug: Res<PlanetDebugConfig>,
    starfield: Res<StarfieldAssets>,
    existing_camera: Query<Entity, With<MainCamera>>,
    mut existing_light: Query<
        (
            Entity,
            &mut DirectionalLight,
            &mut PointLight,
            &mut Transform,
            &mut Visibility,
        ),
        With<SunLight>,
    >,
    existing_skybox: Query<(), With<Skybox>>,
    sun_settings: Res<SunSettings>,
    mut sun_direction: ResMut<SunDirection>,
    mut surface_model: ResMut<PlanetSurfaceModel>,
) {
    sampler_res.0.seed = map.seed;
    surface_model.radius = params.radius;
    surface_model.biome_resolution = DEFAULT_BIOME_MAP_RESOLUTION;

    let target_rotation = sun_rotation_from_params(params.rotation_deg);

    let sun_color = Color::srgb(1.0, 0.93, 0.78);
    let sun_emissive = sun_color.to_linear() * 6.5;
    let sun_distance = sun_visual_distance(params.radius);
    let sun_radius = sun_visual_radius(params.radius);
    let sun_scale = Vec3::splat(sun_radius);
    let sun_point_range = sun_distance * SUN_POINT_RANGE_FACTOR;
    let dir_illuminance = sun_settings.directional_illuminance();
    let point_intensity = sun_settings.point_intensity();

    let sun_direction_vec = target_rotation.mul_vec3(-Vec3::Z);
    let sun_translation = -sun_direction_vec * sun_distance;
    let dir_from_planet = sun_translation.normalize_or_zero();
    if dir_from_planet.length_squared() > 0.0 {
        sun_direction.0 = dir_from_planet;
    }

    spawn_random_planet_inner(
        &mut commands,
        &mut assets.meshes,
        &mut *assets.planet_materials,
        &mut *assets.atmosphere_materials,
        &mut *assets.standard_materials,
        &mut *assets.images,
        &*sampler_res,
        &map,
        &params,
        &settings,
        &debug,
        &*sun_direction,
        &mut *surface_model,
    );
    info!("spawned planet with radius {}", params.radius);

    if let Some((light_entity, mut dir_light, mut point_light, mut transform, mut visibility)) =
        existing_light.iter_mut().next()
    {
        dir_light.color = sun_color;
        dir_light.illuminance = dir_illuminance;
        dir_light.shadows_enabled = true;

        point_light.color = sun_color;
        point_light.intensity = point_intensity;
        point_light.range = sun_point_range;
        point_light.radius = (sun_radius * 0.5).max(1.0);
        point_light.shadows_enabled = false;

        let translation = sun_translation;
        let to_planet = (-translation).normalize_or_zero();
        let rotation = if to_planet.length_squared() > 0.0 {
            Quat::from_rotation_arc(Vec3::NEG_Z, to_planet)
        } else {
            Quat::IDENTITY
        };

        transform.translation = translation;
        transform.scale = sun_scale;
        transform.rotation = rotation;
        *visibility = Visibility::Visible;
        commands.entity(light_entity).insert(NoFrustumCulling);
    } else {
        let translation = sun_translation;
        let mesh = assets.meshes.add(Sphere::new(1.0));
        let material = assets.standard_materials.add(StandardMaterial {
            base_color: sun_color,
            emissive: sun_emissive,
            unlit: true,
            ..default()
        });
        let to_planet = (-translation).normalize_or_zero();
        let sun_rotation = if to_planet.length_squared() > 0.0 {
            Quat::from_rotation_arc(Vec3::NEG_Z, to_planet)
        } else {
            Quat::IDENTITY
        };

        commands.spawn((
            SunLight {
                base_pitch: SUN_BASE_PITCH,
                base_yaw: SUN_BASE_YAW,
            },
            DirectionalLight {
                color: sun_color,
                shadows_enabled: true,
                illuminance: dir_illuminance,
                ..default()
            },
            PointLight {
                color: sun_color,
                intensity: point_intensity,
                range: sun_point_range,
                radius: (sun_radius * 0.5).max(1.0),
                shadows_enabled: false,
                ..default()
            },
            Mesh3d(mesh),
            MeshMaterial3d(material),
            Transform {
                translation,
                rotation: sun_rotation,
                scale: sun_scale,
            },
            GlobalTransform::default(),
            NoFrustumCulling,
            Visibility::Visible,
            InheritedVisibility::default(),
            Name::new("Sun"),
        ));
        info!("spawned sun light");
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
        Mesh3d(assets.meshes.add(Sphere::new(50.0))),
        MeshMaterial3d(assets.standard_materials.add(StandardMaterial {
            base_color: Color::srgb(0.8, 0.2, 0.2),
            ..default()
        })),
        Transform::from_xyz(0.0, params.radius + 60.0, 0.0),
        Name::new("DebugSphere"),
    ));
}

<<<<<<< HEAD
fn load_planet_patch_manifest(mut commands: Commands, asset_server: Res<AssetServer>) {
    match PlanetPatchManifest::scan("assets/planet_patches") {
        Ok(manifest) => {
            let mut cache = PlanetPatchCache::new(manifest);
            cache.preload_face(0, &asset_server);
            info!(
                "planet patch manifest loaded (pages: {}, resident: {})",
                cache.manifest().page_count(),
                cache.resident_count()
            );
            commands.insert_resource(cache);
        }
        Err(err) => warn!("failed to scan planet patch assets: {err}"),
    }
=======
fn cleanup_ingame_world(
    mut commands: Commands,
    cameras: Query<Entity, With<MainCamera>>,
    suns: Query<Entity, With<SunLight>>,
    planets: Query<Entity, With<PlanetTag>>,
    mut planet_entity: ResMut<PlanetEntity>,
    children: Query<&Children>,
) {
    for entity in &cameras {
        despawn_entity_recursive(&mut commands, entity, &children);
    }
    for entity in &suns {
        despawn_entity_recursive(&mut commands, entity, &children);
    }
    for entity in &planets {
        despawn_entity_recursive(&mut commands, entity, &children);
    }
    planet_entity.0 = None;
}

fn despawn_entity_recursive(
    commands: &mut Commands,
    entity: Entity,
    children_q: &Query<&Children>,
) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_entity_recursive(commands, child, children_q);
        }
    }
    commands.entity(entity).despawn();
>>>>>>> 4058b87e56e36fbd9e9e3274857e4a83fb032e63
}

fn sync_sun_with_planet_rotation(
    params: Res<PlanetParams>,
    sun_settings: Res<SunSettings>,
    mut sun_direction: ResMut<SunDirection>,
    mut lights: Query<(
        &SunLight,
        &mut Transform,
        &mut DirectionalLight,
        &mut PointLight,
    )>,
) {
    if !params.is_changed() {
        return;
    }

    let distance = sun_visual_distance(params.radius);
    let radius = sun_visual_radius(params.radius);
    let scale = Vec3::splat(radius);

    let dir_illuminance = sun_settings.directional_illuminance();
    let point_intensity = sun_settings.point_intensity();

    let mut new_direction = None;
    for (sun, mut transform, mut dir_light, mut point_light) in &mut lights {
        let yaw = sun.base_yaw + params.rotation_deg.to_radians();
        let rotation = Quat::from_euler(EulerRot::XYZ, sun.base_pitch, yaw, 0.0);
        let direction = rotation.mul_vec3(-Vec3::Z);
        let translation = -direction * distance;
        let to_planet = (-translation).normalize_or_zero();

        transform.translation = translation;
        transform.scale = scale;
        transform.rotation = if to_planet.length_squared() > 0.0 {
            Quat::from_rotation_arc(Vec3::NEG_Z, to_planet)
        } else {
            Quat::IDENTITY
        };
        dir_light.color = Color::srgb(1.0, 0.93, 0.78);
        dir_light.illuminance = dir_illuminance;
        dir_light.shadows_enabled = true;

        point_light.color = Color::srgb(1.0, 0.93, 0.78);
        point_light.intensity = point_intensity;
        point_light.range = distance * SUN_POINT_RANGE_FACTOR;
        point_light.radius = (radius * 0.5).max(1.0);
        point_light.shadows_enabled = false;

        if translation.length_squared() > 0.0 && new_direction.is_none() {
            new_direction = Some(translation.normalize());
        }
    }

    if let Some(dir) = new_direction {
        sun_direction.0 = dir;
    }
}

fn apply_sun_settings(
    sun_settings: Res<SunSettings>,
    mut lights: Query<
        (
            &mut DirectionalLight,
            &mut PointLight,
            &Transform,
            &mut Visibility,
        ),
        With<SunLight>,
    >,
) {
    if !sun_settings.is_changed() {
        return;
    }
    let dir_illuminance = sun_settings.directional_illuminance();
    let point_intensity = sun_settings.point_intensity();
    for (mut dir_light, mut point_light, transform, mut visibility) in &mut lights {
        dir_light.color = Color::srgb(1.0, 0.93, 0.78);
        dir_light.illuminance = dir_illuminance;
        dir_light.shadows_enabled = true;

        point_light.color = Color::srgb(1.0, 0.93, 0.78);
        point_light.intensity = point_intensity;
        point_light.range = transform.translation.length() * SUN_POINT_RANGE_FACTOR;
        point_light.radius = (transform.scale.x * 0.5).max(1.0);
        point_light.shadows_enabled = false;
        *visibility = Visibility::Visible;
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct SunDirection(pub Vec3);

impl Default for SunDirection {
    fn default() -> Self {
        Self(Vec3::new(0.32, 0.78, 0.54).normalize_or_zero())
    }
}

impl SunDirection {
    #[inline]
    pub fn as_vec4(self) -> Vec4 {
        self.0.extend(0.0)
    }
}

fn enforce_sun_visibility(mut suns: Query<&mut Visibility, With<SunLight>>) {
    for mut visibility in &mut suns {
        if !matches!(*visibility, Visibility::Visible) {
            *visibility = Visibility::Visible;
        }
    }
}

fn update_sun_direction_from_transform(
    mut sun_direction: ResMut<SunDirection>,
    lights: Query<&Transform, With<SunLight>>,
) {
    let Ok(transform) = lights.single() else {
        return;
    };
    let dir = transform.translation.normalize_or_zero();
    if dir.length_squared() > 0.0 && sun_direction.0.distance_squared(dir) > 1e-6 {
        sun_direction.0 = dir;
    }
}

fn sun_rotation_from_params(rotation_deg: f32) -> Quat {
    let yaw = SUN_BASE_YAW + rotation_deg.to_radians();
    Quat::from_euler(EulerRot::XYZ, SUN_BASE_PITCH, yaw, 0.0)
}

