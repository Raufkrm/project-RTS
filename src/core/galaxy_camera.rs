use crate::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::game::world::planet::{PlanetParams, PlanetTag};

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
const ORBIT_ZOOM_RATE: f32 = 0.02;

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
    pub radius: f32,
    pub altitude: f32,
    pub entry_altitude: f32,
    pub longitude: f32,
    pub latitude: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum CameraMode {
    Free,
    Orbit,
}

#[derive(Component, Debug)]
pub struct GalaxyCamera {
    pub mode: CameraMode,
    pub free: FreeState,
    pub orbit: Option<OrbitState>,
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
                mode: CameraMode::Free,
                free,
                orbit: None,
            },
            transform,
        )
    }
}

pub struct GalaxyCameraPlugin;
impl Plugin for GalaxyCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, camera_controller_system);
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
    mut q_cam: Query<(&Camera, &GlobalTransform, &mut Transform, &mut GalaxyCamera)>,
    q_planets: Query<(Entity, &GlobalTransform), With<PlanetTag>>,
    params: Res<PlanetParams>,
) {
    let Ok((camera, cam_global, mut transform, mut state)) = q_cam.single_mut() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let dt = time.delta_secs();

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

    match state.mode {
        CameraMode::Free => {
            let mut free = state.free;

            if buttons.pressed(MouseButton::Right) {
                let sens = mouse_sensitivity_scale(free.distance, params.radius);
                free.yaw = wrap_angle(free.yaw + mouse_delta.x * FREE_ROT_SPEED * sens);
                free.pitch =
                    (free.pitch - mouse_delta.y * FREE_ROT_SPEED * sens).clamp(-1.55, -0.15);
            }

            // WASD/QE planar movement
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
                let speed = FREE_PAN_SPEED * (free.distance / params.radius.max(1.0)).max(1.0);
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
                                nearest_planet(origin, dir, &q_planets, params.radius)
                            {
                                let hit_point = origin + dir * t_hit;
                                let hit_dir = (hit_point - center).normalize();
                                let longitude = hit_dir.z.atan2(hit_dir.x);
                                let latitude =
                                    hit_dir.y.asin().clamp(-ORBIT_LAT_LIMIT, ORBIT_LAT_LIMIT);
                                let current_dist = transform.translation.distance(center);
                                let altitude = (current_dist - params.radius)
                                    .clamp(ORBIT_MIN_ALT, params.radius * ORBIT_MAX_ALT_FACTOR);
                                let new_orbit = OrbitState {
                                    target: entity,
                                    radius: params.radius,
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
                        .clamp(FREE_MIN_DISTANCE, params.radius * FREE_MAX_DISTANCE_FACTOR);
                } else {
                    state.free = free;
                    return;
                }
            }

            state.free = free;
            apply_free_transform(&state.free, &mut transform, dt, true);
        }
        CameraMode::Orbit => {
            let mut orbit = match state.orbit {
                Some(o) => o,
                None => {
                    state.mode = CameraMode::Free;
                    apply_free_transform(&state.free, &mut transform, dt, false);
                    return;
                }
            };

            if keys.just_pressed(KeyCode::Space) {
                state.mode = CameraMode::Free;
                state.orbit = None;
                apply_free_transform(&state.free, &mut transform, dt, false);
                return;
            }

            let Some((_, target_tf)) = q_planets.iter().find(|(e, _)| *e == orbit.target) else {
                state.mode = CameraMode::Free;
                state.orbit = None;
                apply_free_transform(&state.free, &mut transform, dt, false);
                return;
            };
            let center = target_tf.translation();

            if buttons.pressed(MouseButton::Right) {
                let sens = mouse_sensitivity_scale(orbit.altitude + orbit.radius, params.radius);
                orbit.longitude =
                    wrap_angle(orbit.longitude + mouse_delta.x * ORBIT_ROT_SPEED * sens);
                orbit.latitude = (orbit.latitude - mouse_delta.y * ORBIT_ROT_SPEED * sens)
                    .clamp(-ORBIT_LAT_LIMIT, ORBIT_LAT_LIMIT);
            }

            if wheel_sum.abs() > f32::EPSILON {
                let factor = (1.0 - wheel_sum * ORBIT_ZOOM_RATE).clamp(0.4, 1.6);
                orbit.altitude = (orbit.altitude * factor)
                    .clamp(ORBIT_MIN_ALT, orbit.radius * ORBIT_MAX_ALT_FACTOR);

                // Require extra headroom before dropping back to the free camera to avoid rapid re-snaps.
                let exit_altitude = (orbit.entry_altitude + orbit.radius * ORBIT_EXIT_EXTRA_FACTOR)
                    .clamp(ORBIT_MIN_ALT, orbit.radius * ORBIT_MAX_ALT_FACTOR);

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
                    return;
                }
            }

            apply_orbit_transform(&orbit, center, &mut transform, dt, true);

            state.orbit = Some(orbit);
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
    let target_translation = center + dir * (orbit.radius + orbit.altitude);
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

fn wrap_angle(mut a: f32) -> f32 {
    while a > std::f32::consts::PI {
        a -= std::f32::consts::TAU;
    }
    while a < -std::f32::consts::PI {
        a += std::f32::consts::TAU;
    }
    a
}

fn mouse_sensitivity_scale(distance: f32, radius: f32) -> f32 {
    let base = radius.max(1.0);
    let ratio = (distance / base).clamp(0.0, 12.0);
    let t = (ratio / 6.0).clamp(0.0, 1.0);
    let min = 0.2;
    let max = 1.6;
    min + (max - min) * t
}
