// top of file
use crate::app::AppState;
use crate::core::galaxy_camera::{GalaxyCamera, GalaxyCameraPlugin, MainCamera};
use crate::core::planet_debug::PlanetDebugPlugin;
use crate::core::skybox::{Skybox, SkyboxPlugin, StarfieldAssets};
use crate::game::world::planet::{
    spawn_random_planet_inner, sync_planet_material_debug, toggle_planet_wireframe,
    update_planet_lod, PlanetDebugConfig, PlanetParams, PlanetSettings, PlanetSurfaceMaterial,
};
use crate::game::world::sampling::FlatSamplerRes;
use crate::game::world::terrain::MapSettings;
use bevy::{
    math::{primitives::Sphere, EulerRot, Quat, Vec3},
    pbr::{wireframe::WireframePlugin, MaterialPlugin, StandardMaterial},
    prelude::*,
    render::{
        render_resource::AsBindGroup, renderer::RenderDevice, Render, RenderApp, RenderSystems,
    },
};

pub mod ui;
pub mod world;

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
                    apply_sun_settings,
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
    mut existing_light: Query<
        (&mut DirectionalLight, &mut PointLight, &mut Transform),
        With<SunLight>,
    >,
    existing_skybox: Query<(), With<Skybox>>,
    sun_settings: Res<SunSettings>,
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

    let sun_color = Color::srgb(1.0, 0.93, 0.78);
    let sun_emissive = sun_color.to_linear() * 6.5;
    let sun_distance = sun_visual_distance(params.radius);
    let sun_radius = sun_visual_radius(params.radius);
    let sun_scale = Vec3::splat(sun_radius);
    let sun_point_range = sun_distance * SUN_POINT_RANGE_FACTOR;
    let dir_illuminance = sun_settings.directional_illuminance();
    let point_intensity = sun_settings.point_intensity();

    let compute_sun_translation = |rotation: Quat| {
        let direction = rotation.mul_vec3(-Vec3::Z);
        -direction * sun_distance
    };

    if let Some((mut dir_light, mut point_light, mut transform)) = existing_light.iter_mut().next()
    {
        dir_light.color = sun_color;
        dir_light.illuminance = dir_illuminance;
        dir_light.shadows_enabled = true;

        point_light.color = sun_color;
        point_light.intensity = point_intensity;
        point_light.range = sun_point_range;
        point_light.radius = (sun_radius * 0.5).max(1.0);
        point_light.shadows_enabled = false;

        let translation = compute_sun_translation(target_rotation);
        let to_planet = (-translation).normalize_or_zero();
        let rotation = if to_planet.length_squared() > 0.0 {
            Quat::from_rotation_arc(Vec3::NEG_Z, to_planet)
        } else {
            Quat::IDENTITY
        };

        transform.translation = translation;
        transform.scale = sun_scale;
        transform.rotation = rotation;
    } else {
        let translation = compute_sun_translation(target_rotation);
        let mesh = meshes.add(Sphere::new(1.0));
        let material = standard_materials.add(StandardMaterial {
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
    sun_settings: Res<SunSettings>,
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
    }
}

fn apply_sun_settings(
    sun_settings: Res<SunSettings>,
    mut lights: Query<(&mut DirectionalLight, &mut PointLight, &Transform), With<SunLight>>,
) {
    if !sun_settings.is_changed() {
        return;
    }
    let dir_illuminance = sun_settings.directional_illuminance();
    let point_intensity = sun_settings.point_intensity();
    for (mut dir_light, mut point_light, transform) in &mut lights {
        dir_light.color = Color::srgb(1.0, 0.93, 0.78);
        dir_light.illuminance = dir_illuminance;
        dir_light.shadows_enabled = true;

        point_light.color = Color::srgb(1.0, 0.93, 0.78);
        point_light.intensity = point_intensity;
        point_light.range = transform.translation.length() * SUN_POINT_RANGE_FACTOR;
        point_light.radius = (transform.scale.x * 0.5).max(1.0);
        point_light.shadows_enabled = false;
    }
}

fn sun_rotation_from_params(rotation_deg: f32) -> Quat {
    let yaw = SUN_BASE_YAW + rotation_deg.to_radians();
    Quat::from_euler(EulerRot::XYZ, SUN_BASE_PITCH, yaw, 0.0)
}
