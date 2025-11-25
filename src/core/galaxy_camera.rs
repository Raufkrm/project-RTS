use crate::core::surface_model::{dir_to_face_uv, PlanetSurfaceModel, SurfaceCoord};
use crate::game::world::planet::PlanetTag;
use crate::game::world::surface_grid::SurfaceGrid;
use crate::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

const FREE_MIN_DISTANCE: f32 = 40.0;
const FREE_MAX_DISTANCE_FACTOR: f32 = 60.0;
const FREE_ROT_SPEED: f32 = 0.005;
const FREE_PAN_SPEED: f32 = 8.0;
const CAMERA_SMOOTH_SPEED: f32 = 8.0;

const ORBIT_MIN_ALT: f32 = 4.0;
const ORBIT_MAX_ALT_FACTOR: f32 = 6.0;
const ORBIT_ROT_SPEED: f32 = 0.004;
const ORBIT_LAT_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 0.02;
/// Extra altitude (in planet radii) we require before auto-dropping back to free mode.
const ORBIT_EXIT_EXTRA_FACTOR: f32 = 1.2;
pub const CAMERA_SURFACE_CLEARANCE: f32 = 20.0;
const FP_EYE_HEIGHT: f32 = 2.0;
const FP_SPEED: f32 = 15.0;
const FP_LOOK_SPEED: f32 = 0.003;
const FP_PITCH_LIMIT: f32 = 1.3;

#[derive(Resource, Clone)]
pub struct PlanetZoomConfig {
    /// Minimum altitude above surface in meters.
    pub min_altitude: f32,
    /// Maximum altitude above surface in meters.
    pub max_altitude: f32,
    /// Scroll sensitivity for normalized zoom.
    pub scroll_sensitivity: f32,
}

impl Default for PlanetZoomConfig {
    fn default() -> Self {
        Self {
            min_altitude: 5_000.0,
            max_altitude: 2_000_000.0,
            scroll_sensitivity: 0.08,
        }
    }
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Clone, Copy, Debug)]
pub struct FreeState {
    pub focus: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct OrbitState {
    pub target: Entity,
    pub zoom_t: f32,
    pub radius: f32,
    pub altitude: f32,
    pub entry_altitude: f32,
    pub longitude: f32,
    pub latitude: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraMode {
    Orbit,
    Free,
    FirstPerson,
}

#[derive(Component, Debug)]
pub struct GalaxyCamera {
    pub mode: CameraMode,
    pub free: FreeState,
    pub orbit: Option<OrbitState>,
    /// Normalized zoom in [0, 1]: 0 = near surface, 1 = far orbit.
    pub zoom_t: f32,
    /// Cached radius from planet center.
    pub radius: f32,
    pub fp_coord: SurfaceCoord,
    pub fp_yaw: f32,
    pub fp_pitch: f32,
}

impl GalaxyCamera {
    pub fn new(radius: f32) -> (Self, Transform) {
        let free = FreeState {
            focus: Vec3::ZERO,
            distance: radius * 6.0,
            yaw: 0.0,
            pitch: -1.05,
        };
        let mut transform = Transform::default();
        apply_free_transform(&free, &mut transform, 0.0, false);
        (
            GalaxyCamera {
                mode: CameraMode::Orbit,
                free,
                orbit: None,
                zoom_t: 0.5,
                radius: 0.0,
                fp_coord: SurfaceCoord {
                    face: 0,
                    uv: Vec2::new(0.5, 0.5),
                    height: 0.0,
                },
                fp_yaw: 0.0,
                fp_pitch: 0.0,
            },
            transform,
        )
    }
}

pub struct GalaxyCameraPlugin;
impl Plugin for GalaxyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PlanetZoomConfig>()
            .add_systems(Update, camera_controller_system);
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(deprecated)]
fn camera_controller_system(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q_cam: Query<
        (
            &Camera,
            &GlobalTransform,
            &mut Transform,
            &mut GalaxyCamera,
            &mut Projection,
        ),
        With<MainCamera>,
    >,
    q_planets: Query<(Entity, &GlobalTransform), With<PlanetTag>>,
    surface_model: Res<PlanetSurfaceModel>,
    surface_grid: Res<SurfaceGrid>,
    zoom_cfg: Res<PlanetZoomConfig>,
) {
    let Ok((camera, cam_global, mut transform, mut state, mut projection)) = q_cam.single_mut()
    else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let dt = time.delta_secs();
    let planet_radius = surface_model.radius.max(1.0);

    let mut mouse_delta = Vec2::ZERO;
    for ev in motion.read() {
        mouse_delta += ev.delta;
    }

    let mut wheel_sum = 0.0_f32;
    for ev in wheel.read() {
        let delta = match ev.unit {
            MouseScrollUnit::Line => ev.y,
            MouseScrollUnit::Pixel => ev.y / 120.0,
        };
        wheel_sum += delta;
    }

    // Toggle FP mode with F
    if keys.just_pressed(KeyCode::KeyF) {
        match state.mode {
            CameraMode::FirstPerson => {
                if let Some((planet_entity, planet_tf)) = q_planets.iter().next() {
                    let center = planet_tf.translation();
                    let radius = transform.translation.distance(center).max(planet_radius);
                    let radius_clamped = radius.clamp(
                        surface_model.radius + zoom_cfg.min_altitude,
                        surface_model.radius + zoom_cfg.max_altitude,
                    );
                    let altitude =
                        radius_to_altitude(radius_clamped, &surface_model).max(0.0);
                    let dir_to_center = (center - transform.translation).normalize_or_zero();
                    let longitude = dir_to_center.z.atan2(dir_to_center.x);
                    let latitude =
                        dir_to_center.y.asin().clamp(-ORBIT_LAT_LIMIT, ORBIT_LAT_LIMIT);
                    state.mode = CameraMode::Orbit;
                    state.zoom_t =
                        radius_to_zoom_t(radius_clamped, &zoom_cfg, &surface_model);
                    state.radius = radius_clamped;
                    state.orbit = Some(OrbitState {
                        target: planet_entity,
                        zoom_t: state.zoom_t,
                        radius: radius_clamped,
                        altitude,
                        entry_altitude: altitude,
                        longitude,
                        latitude,
                    });
                } else {
                    state.mode = CameraMode::Orbit;
                }
            }
            _ => {
                let dir = transform.translation.normalize_or_zero();
                let (face, uv) = dir_to_face_uv(dir);
                let height = surface_grid.sample_height_uv(face, uv);
                state.fp_coord.face = face;
                state.fp_coord.uv = uv;
                state.fp_coord.height = height;

                let forward = transform.forward().as_vec3();
                let up = transform.translation.normalize_or_zero();
                let forward_t = (forward - up * forward.dot(up)).normalize_or_zero();
                let yaw = forward_t.z.atan2(forward_t.x);
                let pitch = forward.dot(up).clamp(-1.0, 1.0).asin();
                state.fp_yaw = yaw;
                state.fp_pitch = pitch;
                state.mode = CameraMode::FirstPerson;
                state.orbit = None;
            }
        }
    }

    match state.mode {
        CameraMode::FirstPerson => {
            if mouse_delta.length_squared() > 0.0 {
                state.fp_yaw -= mouse_delta.x * FP_LOOK_SPEED;
                state.fp_pitch =
                    (state.fp_pitch - mouse_delta.y * FP_LOOK_SPEED).clamp(
                        -FP_PITCH_LIMIT,
                        FP_PITCH_LIMIT,
                    );
            }

            let height = surface_grid.sample_height_uv(state.fp_coord.face, state.fp_coord.uv);
            state.fp_coord.height = height;

            let feet_world = surface_model.surface_to_world(state.fp_coord);
            let up = feet_world.normalize_or_zero();

            let yaw = state.fp_yaw;
            let pitch = state.fp_pitch;
            let forward_local = Vec3::new(
                pitch.cos() * yaw.cos(),
                pitch.sin(),
                pitch.cos() * yaw.sin(),
            );
            let forward = (forward_local - up * forward_local.dot(up)).normalize_or_zero();
            let right = up.cross(forward).normalize_or_zero();

            let mut move_dir = Vec3::ZERO;
            if keys.pressed(KeyCode::KeyW) {
                move_dir += forward;
            }
            if keys.pressed(KeyCode::KeyS) {
                move_dir -= forward;
            }
            if keys.pressed(KeyCode::KeyD) {
                move_dir += right;
            }
            if keys.pressed(KeyCode::KeyA) {
                move_dir -= right;
            }
            if move_dir.length_squared() > 0.0 {
                move_dir = move_dir.normalize();
            }

            let dt = time.delta_secs();
            let step_world = move_dir * FP_SPEED * dt;
            let feet_world_new = if move_dir.length_squared() > 0.0 {
                feet_world + step_world
            } else {
                feet_world
            };

            let dir = feet_world_new.normalize_or_zero();
            let (face, uv) = dir_to_face_uv(dir);
            let new_height = surface_grid.sample_height_uv(face, uv);
            state.fp_coord.face = face;
            state.fp_coord.uv = uv;
            state.fp_coord.height = new_height;

            let feet_world_final = surface_model.surface_to_world(state.fp_coord);
            let up_final = feet_world_final.normalize_or_zero();
            let eye_world = feet_world_final + up_final * FP_EYE_HEIGHT;

            transform.translation = eye_world;
            transform.look_at(eye_world + forward * 10.0, up_final);
            transform.scale = Vec3::ONE;

            if let Projection::Perspective(persp) = &mut *projection {
                persp.near = 0.5;
                persp.fov = 60.0_f32.to_radians();
            }

            state.radius = eye_world.length();
            state.zoom_t = radius_to_zoom_t(state.radius, &zoom_cfg, &surface_model);
        }
        CameraMode::Free => {
            let mut free = state.free;

            if buttons.pressed(MouseButton::Right) {
                free.yaw = wrap_angle(free.yaw + mouse_delta.x * FREE_ROT_SPEED);
                free.pitch = (free.pitch - mouse_delta.y * FREE_ROT_SPEED).clamp(-1.55, -0.15);
            }

            let forward = direction_from_yaw_pitch(free.yaw, free.pitch);
            let forward_flat = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
            let right = forward_flat.cross(Vec3::Y).normalize_or_zero();
            let mut wish = Vec3::ZERO;
            if keys.pressed(KeyCode::KeyW) || keys.pressed(KeyCode::ArrowUp) {
                wish += forward_flat;
            }
            if keys.pressed(KeyCode::KeyS) || keys.pressed(KeyCode::ArrowDown) {
                wish -= forward_flat;
            }
            if keys.pressed(KeyCode::KeyD) || keys.pressed(KeyCode::ArrowRight) {
                wish += right;
            }
            if keys.pressed(KeyCode::KeyA) || keys.pressed(KeyCode::ArrowLeft) {
                wish -= right;
            }
            if keys.pressed(KeyCode::KeyQ) {
                wish.y -= 1.0;
            }
            if keys.pressed(KeyCode::KeyE) {
                wish.y += 1.0;
            }
            if wish.length_squared() > 0.0 {
                let speed = FREE_PAN_SPEED * (free.distance / planet_radius).max(1.0);
                free.focus += wish.normalize() * speed * time.delta_secs();
            }

            if wheel_sum.abs() > f32::EPSILON {
                let mut snapped = false;
                if wheel_sum > 0.0 {
                    if let Some(cursor) = window.cursor_position() {
                        if let Ok(ray) = camera.viewport_to_world(cam_global, cursor) {
                            let origin = ray.origin;
                            let dir = ray.direction.as_vec3();
                            if let Some((entity, center, t_hit)) =
                                nearest_planet(origin, dir, &q_planets, planet_radius)
                            {
                                let hit_point = origin + dir * t_hit;
                                let hit_dir = (hit_point - center).normalize();
                                let longitude = hit_dir.z.atan2(hit_dir.x);
                                let latitude =
                                    hit_dir.y.asin().clamp(-ORBIT_LAT_LIMIT, ORBIT_LAT_LIMIT);
                                let current_dist = transform.translation.distance(center);
                                let (min_r, max_r) =
                                    zoom_radius_bounds(&surface_model, &zoom_cfg);
                                let min_alt = zoom_cfg.min_altitude;
                                let max_alt = zoom_cfg.max_altitude;
                                let mut radius = current_dist.clamp(min_r, max_r);
                                let altitude = radius_to_altitude(radius, &surface_model)
                                    .max(0.0)
                                    .clamp(min_alt, max_alt);
                                radius = surface_model.radius + altitude;
                                let zoom_t =
                                    radius_to_zoom_t(radius, &zoom_cfg, &surface_model);
                                state.zoom_t = zoom_t;
                                state.radius = radius;
                                let new_orbit = OrbitState {
                                    target: entity,
                                    zoom_t,
                                    radius,
                                    altitude,
                                    entry_altitude: altitude,
                                    longitude,
                                    latitude,
                                };
                                apply_orbit_transform(
                                    &new_orbit,
                                    center,
                                    &mut transform,
                                    dt,
                                    false,
                                );
                                state.mode = CameraMode::Orbit;
                                state.orbit = Some(new_orbit);
                                snapped = true;
                            }
                        }
                    }
                }

                if !snapped {
                    free.distance = (free.distance - wheel_sum * (free.distance * 0.2))
                        .clamp(FREE_MIN_DISTANCE, planet_radius * FREE_MAX_DISTANCE_FACTOR);
                } else {
                    state.free = free;
                }
            }

            if state.mode == CameraMode::Free {
                state.free = free;
                apply_free_transform(&state.free, &mut transform, dt, true);
                state.radius = 0.0;
                if let Projection::Perspective(persp) = &mut *projection {
                    persp.near = 0.1;
                }
            }
        }
        CameraMode::Orbit => {
            let mut orbit_active = true;
            let mut orbit = match state.orbit {
                Some(o) => o,
                None => {
                    if let Some((planet_entity, planet_tf)) = q_planets.iter().next() {
                        let center = planet_tf.translation();
                        let radius = transform.translation.distance(center).max(planet_radius);
                        let radius_clamped = radius.clamp(
                            surface_model.radius + zoom_cfg.min_altitude,
                            surface_model.radius + zoom_cfg.max_altitude,
                        );
                        let altitude =
                            radius_to_altitude(radius_clamped, &surface_model).max(0.0);
                        let dir_to_center =
                            (center - transform.translation).normalize_or_zero();
                        let longitude = dir_to_center.z.atan2(dir_to_center.x);
                        let latitude = dir_to_center
                            .y
                            .asin()
                            .clamp(-ORBIT_LAT_LIMIT, ORBIT_LAT_LIMIT);
                        OrbitState {
                            target: planet_entity,
                            zoom_t: radius_to_zoom_t(
                                radius_clamped,
                                &zoom_cfg,
                                &surface_model,
                            ),
                            radius: radius_clamped,
                            altitude,
                            entry_altitude: altitude,
                            longitude,
                            latitude,
                        }
                    } else {
                        state.mode = CameraMode::Free;
                        apply_free_transform(&state.free, &mut transform, dt, false);
                        orbit_active = false;
                        OrbitState {
                            target: Entity::PLACEHOLDER,
                            zoom_t: 0.0,
                            radius: 0.0,
                            altitude: 0.0,
                            entry_altitude: 0.0,
                            longitude: 0.0,
                            latitude: 0.0,
                        }
                    }
                }
            };

            if orbit_active && keys.just_pressed(KeyCode::Space) {
                state.mode = CameraMode::Free;
                state.orbit = None;
                apply_free_transform(&state.free, &mut transform, dt, false);
                state.radius = 0.0;
                orbit_active = false;
            }

            if orbit_active {
                let maybe_target = q_planets.iter().find(|(e, _)| *e == orbit.target);
                if let Some((_, target_tf)) = maybe_target {
                    let center = target_tf.translation();

                    if buttons.pressed(MouseButton::Right) {
                        orbit.longitude =
                            wrap_angle(orbit.longitude + mouse_delta.x * ORBIT_ROT_SPEED);
                        orbit.latitude = (orbit.latitude - mouse_delta.y * ORBIT_ROT_SPEED)
                            .clamp(-ORBIT_LAT_LIMIT, ORBIT_LAT_LIMIT);
                    }

                    let (min_r, max_r) = zoom_radius_bounds(&surface_model, &zoom_cfg);
                    let min_alt = zoom_cfg.min_altitude;
                    let max_alt = zoom_cfg.max_altitude;

                    if orbit.radius <= 0.0 {
                        let radius = transform.translation.distance(center).max(planet_radius);
                        orbit.radius = radius.clamp(min_r, max_r);
                        orbit.zoom_t = radius_to_zoom_t(orbit.radius, &zoom_cfg, &surface_model);
                        orbit.altitude = radius_to_altitude(orbit.radius, &surface_model)
                            .clamp(min_alt, max_alt);
                        state.zoom_t = orbit.zoom_t;
                        state.radius = orbit.radius;
                    }
                    if wheel_sum.abs() > f32::EPSILON {
                        orbit.zoom_t = (orbit.zoom_t - wheel_sum * zoom_cfg.scroll_sensitivity)
                            .clamp(0.0, 1.0);
                    }
                    orbit.radius = zoom_t_to_radius(orbit.zoom_t, &zoom_cfg, &surface_model)
                        .clamp(min_r, max_r);
                    orbit.altitude =
                        radius_to_altitude(orbit.radius, &surface_model).clamp(min_alt, max_alt);
                    state.zoom_t = orbit.zoom_t;
                    state.radius = orbit.radius;

                    let exit_altitude =
                        (orbit.entry_altitude + planet_radius * ORBIT_EXIT_EXTRA_FACTOR)
                            .clamp(min_alt, max_alt);

                    if wheel_sum < 0.0 && orbit.altitude > exit_altitude {
                        let forward = (center - transform.translation).normalize();
                        let yaw = forward.z.atan2(forward.x);
                        let pitch = forward.y.asin().clamp(-1.55, -0.15);
                        state.free = FreeState {
                            focus: center,
                            distance: transform.translation.distance(center),
                            yaw,
                            pitch,
                        };
                        state.mode = CameraMode::Free;
                        state.orbit = None;
                        apply_free_transform(&state.free, &mut transform, dt, false);
                        state.radius = 0.0;
                        orbit_active = false;
                    }

                    if orbit_active {
                        apply_orbit_transform(&orbit, center, &mut transform, dt, true);
                        if let Projection::Perspective(persp) = &mut *projection {
                            persp.near = 0.1;
                        }
                        state.orbit = Some(orbit);
                    }
                } else {
                    state.mode = CameraMode::Free;
                    state.orbit = None;
                    apply_free_transform(&state.free, &mut transform, dt, false);
                    state.radius = 0.0;
                }
            }
        }
    }

    // Final clamp to keep camera above the surface
    let pos = transform.translation;
    let radius = pos.length();
    if radius > 0.0 {
        let dir = pos / radius;
        let height = surface_model.sample_height_at_dir(dir, &surface_grid);
        let min_radius = surface_model.radius + height + CAMERA_SURFACE_CLEARANCE;
        if radius < min_radius {
            let clamped = dir * min_radius;
            transform.translation = clamped;
            match state.mode {
                CameraMode::FirstPerson => {
                    let (face, uv) = dir_to_face_uv(dir);
                    state.fp_coord.face = face;
                    state.fp_coord.uv = uv;
                    state.fp_coord.height = height;
                }
                CameraMode::Orbit => {
                    let mut new_radius = None;
                    let mut new_zoom = None;
                    if let Some(ref mut orbit) = state.orbit {
                        orbit.radius = min_radius;
                        orbit.altitude = min_radius - surface_model.radius;
                        new_radius = Some(orbit.radius);
                        new_zoom =
                            Some(radius_to_zoom_t(orbit.radius, &zoom_cfg, &surface_model));
                    }
                    if let Some(r) = new_radius {
                        state.radius = r;
                    }
                    if let Some(z) = new_zoom {
                        state.zoom_t = z;
                    }
                }
                CameraMode::Free => {
                    state.free.distance = transform.translation.length();
                }
            }
        }
    }
}

fn apply_free_transform(free: &FreeState, transform: &mut Transform, dt: f32, smooth: bool) {
    let forward = direction_from_yaw_pitch(free.yaw, free.pitch);
    let target_translation = free.focus - forward * free.distance;
    let mut target = Transform::from_translation(target_translation);
    target.look_at(free.focus, Vec3::Y);
    apply_transform(transform, target_translation, target.rotation, dt, smooth);
}

fn apply_orbit_transform(
    orbit: &OrbitState,
    center: Vec3,
    transform: &mut Transform,
    dt: f32,
    smooth: bool,
) {
    let dir = direction_from_lat_lon(orbit.latitude, orbit.longitude);
    let target_translation = center + dir * orbit.radius;
    let mut target = Transform::from_translation(target_translation);
    target.look_at(center, Vec3::Y);
    apply_transform(transform, target_translation, target.rotation, dt, smooth);
}

fn apply_transform(
    transform: &mut Transform,
    target_translation: Vec3,
    target_rotation: Quat,
    dt: f32,
    smooth: bool,
) {
    if smooth {
        let alpha = smoothing_alpha(dt);
        transform.translation = transform.translation.lerp(target_translation, alpha);
        transform.rotation = transform.rotation.slerp(target_rotation, alpha).normalize();
    } else {
        transform.translation = target_translation;
        transform.rotation = target_rotation;
    }
    transform.scale = Vec3::ONE;
}

fn smoothing_alpha(dt: f32) -> f32 {
    1.0 - (-CAMERA_SMOOTH_SPEED * dt).exp()
}

fn direction_from_yaw_pitch(yaw: f32, pitch: f32) -> Vec3 {
    let cos_pitch = pitch.cos();
    Vec3::new(yaw.sin() * cos_pitch, pitch.sin(), -yaw.cos() * cos_pitch).normalize()
}

fn direction_from_lat_lon(lat: f32, lon: f32) -> Vec3 {
    let cos_lat = lat.cos();
    Vec3::new(cos_lat * lon.cos(), lat.sin(), cos_lat * lon.sin()).normalize()
}

fn ray_sphere_t(origin: Vec3, dir: Vec3, center: Vec3, radius: f32) -> Option<f32> {
    let oc = origin - center;
    let b = oc.dot(dir);
    let c = oc.length_squared() - radius * radius;
    let discriminant = b * b - c;
    if discriminant < 0.0 {
        None
    } else {
        let sqrt_d = discriminant.sqrt();
        let t1 = -b - sqrt_d;
        let t2 = -b + sqrt_d;
        if t1 > 0.0 {
            Some(t1)
        } else if t2 > 0.0 {
            Some(t2)
        } else {
            None
        }
    }
}

fn nearest_planet(
    origin: Vec3,
    dir: Vec3,
    planets: &Query<(Entity, &GlobalTransform), With<PlanetTag>>,
    radius: f32,
) -> Option<(Entity, Vec3, f32)> {
    let mut best: Option<(Entity, Vec3, f32)> = None;
    for (entity, tf) in planets.iter() {
        let center = tf.translation();
        if let Some(t) = ray_sphere_t(origin, dir, center, radius) {
            if t > 0.0 {
                match best {
                    Some((_, _, best_t)) if t >= best_t => {}
                    _ => best = Some((entity, center, t)),
                }
            }
        }
    }
    best
}

fn zoom_t_to_radius(
    zoom_t: f32,
    cfg: &PlanetZoomConfig,
    surface: &PlanetSurfaceModel,
) -> f32 {
    let base = surface.radius;
    let min_r = base + cfg.min_altitude;
    let max_r = base + cfg.max_altitude;
    let t = zoom_t.clamp(0.0, 1.0);
    let ln_min = min_r.ln();
    let ln_max = max_r.ln();
    let ln_r = ln_min + t * (ln_max - ln_min);
    ln_r.exp()
}

fn radius_to_zoom_t(
    radius: f32,
    cfg: &PlanetZoomConfig,
    surface: &PlanetSurfaceModel,
) -> f32 {
    let base = surface.radius;
    let min_r = base + cfg.min_altitude;
    let max_r = base + cfg.max_altitude;
    let clamped = radius.clamp(min_r, max_r);
    let ln_min = min_r.ln();
    let ln_max = max_r.ln();
    if (ln_max - ln_min).abs() <= f32::EPSILON {
        0.0
    } else {
        ((clamped.ln() - ln_min) / (ln_max - ln_min)).clamp(0.0, 1.0)
    }
}

fn radius_to_altitude(radius: f32, surface: &PlanetSurfaceModel) -> f32 {
    radius - surface.radius
}

fn zoom_radius_bounds(
    surface: &PlanetSurfaceModel,
    cfg: &PlanetZoomConfig,
) -> (f32, f32) {
    let base = surface.radius;
    let min_r = base + cfg.min_altitude;
    let max_r = base + cfg.max_altitude;
    (min_r, max_r)
}

fn wrap_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}
