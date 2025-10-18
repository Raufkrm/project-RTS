use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::f32::consts::PI;

use crate::app::AppState;
use crate::game::world::planet::PlanetParams;

// ──────────────────────────────────────────────────────────────────────────────
// Plugin
// ──────────────────────────────────────────────────────────────────────────────
pub struct EditorCameraPlugin;
impl Plugin for EditorCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            (despawn_existing_cameras, spawn_camera),
        )
        .add_systems(OnExit(AppState::InGame), despawn_camera)
        .add_systems(Update, update_camera.run_if(in_state(AppState::InGame)));
    }
}

// ──────────────────────────────────────────────────────────────────────────────
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CameraMode {
    Orbit,       // far: orbit planet center
    Surface,     // near: surface anchor + pan/orbit
    FirstPerson, // on foot
}

// Sensitivity & clamps
const DEG: f32 = PI / 180.0;
const CINEMATIC_PITCH: f32 = -65.0 * DEG;
const SURF_PITCH_CLAMP: (f32, f32) = (-80.0 * DEG, -10.0 * DEG);
const FP_PITCH_CLAMP: (f32, f32) = (-85.0 * DEG, 85.0 * DEG);

const FP_ENTER_ALT: f32 = 1.2; // meters above ground to enter FP
const FP_EXIT_ALT: f32 = 2.0; // meters to exit FP back to Surface

// Orbit thresholds scale with planet radius (works for big/small planets)
const ORBIT_ENTER_FRAC: f32 = 3.0; // Surface → Orbit when altitude ≥ r * 3
const ORBIT_EXIT_FRAC: f32 = 2.2; // Orbit → Surface when distance ≤ r + r*2.2

const ROT_SENS: f32 = 0.0040; // rad/pixel for yaw/pitch
const PAN_BASE: f32 = 0.0020; // per-pixel → radians, scaled by altitude/r
const MAX_PX_STEP: f32 = 50.0; // clamp per-frame mouse delta
                               // per-notch zoom rates (tiny, multiplicative)
const ZOOM_RATE_SURF: f32 = 0.06; // altitude scale per notch
const ZOOM_RATE_FP: f32 = 0.06;
const ZOOM_RATE_ORBIT: f32 = 0.08; // orbit-distance scale per notch

// smoothing (critically damped)
const TAU_ROT: f32 = 0.06;
const TAU_POS: f32 = 0.06;
const TAU_ZOOM: f32 = 0.08;

// first-person speed
const FP_SPEED: f32 = 5.0;

// Allow very large zoom-outs (factor * planet radius)
const MAX_ORBIT_FACTOR: f32 = 5000.0;

// ──────────────────────────────────────────────────────────────────────────────
// Helpers (math & smoothing)
// ──────────────────────────────────────────────────────────────────────────────
#[inline]
fn solve_surface_angles(up: Vec3, forward: Vec3) -> (f32, f32) {
    // express `forward` in the local basis (east, up, -north)
    let (east, north) = tangent_frame(up);
    let x = forward.dot(east); // toward east
    let y = forward.dot(up); // up component
    let z = forward.dot(-north); // forward along -north (our "forward" when yaw=pitch=0)

    // From our generator: forward = R_up(yaw) * R_east(pitch) * (0,0,1) in (east,up,-north)
    // So: x = sin(yaw)*cos(pitch),  z = cos(yaw)*cos(pitch),  y = sin(pitch)
    let pitch = y.clamp(-1.0, 1.0).asin();
    let yaw = x.atan2(z);
    (wrap(yaw), pitch)
}

#[inline]
fn wrap(mut a: f32) -> f32 {
    while a > PI {
        a -= 2.0 * PI
    }
    while a < -PI {
        a += 2.0 * PI
    }
    a
}
#[inline]
fn clamp_pitch(p: f32, lo: f32, hi: f32) -> f32 {
    p.clamp(lo + 1e-3, hi - 1e-3)
}
#[inline]
fn tangent_frame(up: Vec3) -> (Vec3, Vec3) {
    let ref_up = if up.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let east = up.cross(ref_up).normalize_or_zero();
    let north = east.cross(up).normalize_or_zero();
    (east, north)
}
#[inline]
fn rotate_dir(dir: Vec3, axis: Vec3, angle: f32) -> Vec3 {
    (Quat::from_axis_angle(axis.normalize_or_zero(), angle) * dir).normalize()
}
#[inline]
fn smooth_to(current: f32, target: f32, dt: f32, tau: f32) -> f32 {
    let a = (-dt / tau.max(1e-5)).exp();
    target + (current - target) * a
}
#[inline]
fn smooth_to_v(current: Vec3, target: Vec3, dt: f32, tau: f32) -> Vec3 {
    let a = (-dt / tau.max(1e-5)).exp();
    target + (current - target) * a
}
#[inline]
fn ray_sphere_hit(eye: Vec3, dir: Vec3, r: f32) -> Option<Vec3> {
    // solve ||eye + t*dir|| = r, t>0
    let b = eye.dot(dir);
    let c = eye.length_squared() - r * r;
    let d = b * b - c;
    if d < 0.0 {
        return None;
    }
    let s = d.sqrt();
    let t1 = -b - s;
    let t2 = -b + s;
    let t = if t1 > 0.0 {
        t1
    } else if t2 > 0.0 {
        t2
    } else {
        return None;
    };
    Some(eye + dir * t)
}

// ──────────────────────────────────────────────────────────────────────────────
// Component
// ──────────────────────────────────────────────────────────────────────────────
#[derive(Component)]
pub struct GameCamera {
    pub yaw: f32,
    pub pitch: f32,

    // Orbit
    pub orbit_dist: f32,
    pub target_orbit_dist: f32,

    // Surface / FP
    pub focus: Vec3,
    pub target_focus: Vec3,
    pub altitude: f32,
    pub target_altitude: f32,

    pub mode: CameraMode,

    // Interaction
    pub last_cursor: Option<Vec2>,
    pub rotating: bool,
    pub panning: bool,
    pub rotate_anchor: Option<Vec3>, // ground hit captured on MMB press
}

// keep older code working that still refers to `EditorCamera`
pub use GameCamera as EditorCamera;

// ──────────────────────────────────────────────────────────────────────────────
// Lifecycle
// ──────────────────────────────────────────────────────────────────────────────
fn despawn_existing_cameras(mut commands: Commands, cams: Query<Entity, With<Camera3d>>) {
    for e in &cams {
        commands.entity(e).despawn();
    }
}

fn spawn_camera(mut commands: Commands, planet: Option<Res<PlanetParams>>) {
    // Sun
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 25_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.7, 0.0)),
        Name::new("Sun"),
    ));

    // Start near surface on +Z with cinematic tilt
    let r = planet.as_ref().map(|p| p.radius).unwrap_or(6.0);
    let start_alt = 8.0;
    let focus = Vec3::new(0.0, 0.0, r);
    let yaw = 0.0;
    let pitch = CINEMATIC_PITCH;

    let up = focus.normalize();
    let (east, north) = tangent_frame(up);
    let forward = (Quat::from_axis_angle(up, yaw) * Quat::from_axis_angle(east, pitch)) * (-north);
    let eye = focus + up * start_alt;

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(eye).looking_to(forward, up),
        GameCamera {
            yaw,
            pitch,
            orbit_dist: r + 60.0,
            target_orbit_dist: r + 60.0,
            focus,
            target_focus: focus,
            altitude: start_alt,
            target_altitude: start_alt,
            mode: CameraMode::Surface,
            last_cursor: None,
            rotating: false,
            panning: false,
            rotate_anchor: None,
        },
        Name::new("GameCamera"),
    ));
}

fn despawn_camera(
    mut commands: Commands,
    q_cam: Query<Entity, With<GameCamera>>,
    q_light: Query<Entity, With<DirectionalLight>>,
) {
    for e in q_cam.iter().chain(q_light.iter()) {
        commands.entity(e).despawn();
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Update
// ──────────────────────────────────────────────────────────────────────────────
fn update_camera(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q: Query<(&mut Transform, &mut GameCamera)>,
    planet: Option<Res<PlanetParams>>,
) {
    let dt = time.delta_secs();
    let Ok(window) = windows.single() else {
        return;
    };
    let cursor = window.cursor_position();

    let (mut t, mut cam) = match q.single_mut() {
        Ok(v) => v,
        Err(_) => return,
    };
    let r = planet.as_ref().map(|p| p.radius).unwrap_or(6.0);
    let orbit_enter_alt = r * ORBIT_ENTER_FRAC;
    let orbit_exit_alt = r * ORBIT_EXIT_FRAC;

    // Wheel zoom (aggregated)
    let mut zoom_units = 0.0f32;
    for evt in wheel.read() {
        match evt.unit {
            MouseScrollUnit::Line => {
                // some mice send big values; we only want direction per notch
                zoom_units += evt.y.signum();
            }
            MouseScrollUnit::Pixel => {
                // 120px is a common OS notch; keep small if smooth wheel
                zoom_units += (evt.y / 120.0).clamp(-1.0, 1.0);
            }
        }
    }
    zoom_units = zoom_units.clamp(-2.0, 2.0);

    // Mouse delta (clamped)
    let mut delta = if let (Some(prev), Some(now)) = (cam.last_cursor, cursor) {
        now - prev
    } else {
        Vec2::ZERO
    };
    if delta.length_squared() > 0.0 {
        delta.x = delta.x.clamp(-MAX_PX_STEP, MAX_PX_STEP);
        delta.y = delta.y.clamp(-MAX_PX_STEP, MAX_PX_STEP);
    }

    // Buttons
    let mmb_just = buttons.just_pressed(MouseButton::Middle);
    let rmb_just = buttons.just_pressed(MouseButton::Right);
    let mmb = buttons.pressed(MouseButton::Middle);
    let rmb = buttons.pressed(MouseButton::Right);

    // Current camera forward
    let eye = t.translation;
    let forward_cam = (t.rotation * Vec3::NEG_Z).normalize();

    // Stable anchor on MMB press
    if mmb_just {
        cam.rotating = true;
        cam.panning = false;
        cam.rotate_anchor = ray_sphere_hit(eye, forward_cam, r).or(Some(cam.target_focus));
        delta = Vec2::ZERO;
    }
    if rmb_just {
        cam.panning = true;
        cam.rotating = false;
        delta = Vec2::ZERO;
    }
    cam.last_cursor = cursor;

    // Mode transitions driven by zoom (with hysteresis)
    match cam.mode {
        CameraMode::Surface => {
            if zoom_units != 0.0 {
                // factor < 1 => zoom in (lower altitude), > 1 => zoom out
                let factor = (1.0 - zoom_units * ZOOM_RATE_SURF).clamp(0.5, 1.5);
                cam.target_altitude =
                    (cam.target_altitude * factor).clamp(FP_ENTER_ALT, orbit_enter_alt * 1.25);
            }

            if cam.target_altitude <= FP_ENTER_ALT {
                cam.mode = CameraMode::FirstPerson;
            }
            if cam.target_altitude >= orbit_enter_alt {
                cam.mode = CameraMode::Orbit;
                cam.target_orbit_dist = r + cam.target_altitude;
            }
        }
        CameraMode::FirstPerson => {
            if zoom_units != 0.0 {
                let factor = (1.0 - zoom_units * ZOOM_RATE_FP).clamp(0.5, 1.5);
                cam.target_altitude = (cam.target_altitude * factor)
                    .clamp(FP_ENTER_ALT, orbit_enter_alt.max(FP_EXIT_ALT));
            }

            if cam.target_altitude >= FP_EXIT_ALT {
                cam.mode = CameraMode::Surface;
            }
        }
        CameraMode::Orbit => {
            if zoom_units != 0.0 {
                let factor = (1.0 - zoom_units * ZOOM_RATE_ORBIT).clamp(0.5, 1.5);
                cam.target_orbit_dist =
                    (cam.target_orbit_dist * factor).clamp(r + FP_EXIT_ALT, r * MAX_ORBIT_FACTOR);
            }

            if cam.target_orbit_dist <= r + orbit_exit_alt {
                // derive surface state from the current orbit view (no jump)
                let fwd = (t.rotation * Vec3::NEG_Z).normalize();
                let hit = ray_sphere_hit(eye, fwd, r).unwrap_or_else(|| eye.normalize() * r);

                let up = hit.normalize();
                let (yaw, mut pitch) = solve_surface_angles(up, fwd);
                // clamp to surface limits
                pitch = clamp_pitch(pitch, SURF_PITCH_CLAMP.0, SURF_PITCH_CLAMP.1);

                // altitude from current distance (feels continuous)
                let alt_now = (t.translation.length() - r)
                    .max(FP_ENTER_ALT)
                    .min(orbit_enter_alt);

                // initialize Surface state to match the current view
                cam.mode = CameraMode::Surface;
                cam.focus = hit;
                cam.target_focus = hit;
                cam.yaw = yaw;
                cam.pitch = pitch;
                cam.altitude = alt_now;
                cam.target_altitude = alt_now;

                // clear interaction state so there’s no leftover drag
                cam.panning = false;
                cam.rotating = false;
                cam.rotate_anchor = None;

                // apply pose immediately (prevents a visible "pop" on next frame)
                let (east, north) = tangent_frame(up);
                let forward = (Quat::from_axis_angle(up, cam.yaw)
                    * Quat::from_axis_angle(east, cam.pitch))
                    * (-north);
                let eye = hit + up * alt_now;

                t.translation = eye;
                t.look_to(forward, up);
                return; // we finished the handoff this frame
            }
        }
    }

    // Mode behavior
    match cam.mode {
        // ─────────────────────────── ORBIT ───────────────────────────
        CameraMode::Orbit => {
            // RMB rotates around center
            if rmb {
                cam.yaw = wrap(cam.yaw - delta.x * ROT_SENS);
                cam.pitch = clamp_pitch(cam.pitch - delta.y * ROT_SENS, -1.54, 1.54);
            }

            cam.orbit_dist = smooth_to(cam.orbit_dist, cam.target_orbit_dist, dt, TAU_ZOOM);

            let dir = Quat::from_euler(EulerRot::YXZ, cam.yaw, cam.pitch, 0.0) * Vec3::NEG_Z;
            let eye = -dir * cam.orbit_dist;
            t.translation = eye;
            t.look_at(Vec3::ZERO, Vec3::Y);
        }

        // ───────────────────────── SURFACE ───────────────────────────
        CameraMode::Surface => {
            // keep focus on surface
            let dir = cam.target_focus.normalize();
            cam.target_focus = dir * r;

            // MMB rotate around captured anchor
            if cam.rotating && mmb {
                let anchor = cam.rotate_anchor.unwrap_or(cam.target_focus);
                cam.target_focus = anchor;
                cam.yaw = wrap(cam.yaw - delta.x * ROT_SENS);
                cam.pitch = clamp_pitch(
                    cam.pitch - delta.y * ROT_SENS,
                    SURF_PITCH_CLAMP.0,
                    SURF_PITCH_CLAMP.1,
                );
            }

            if cam.panning && rmb {
                let up0 = cam.target_focus.normalize();

                // convert pixels to small angular displacements
                let px_to_rad = (cam.target_altitude / r) * PAN_BASE;
                let yaw_step = delta.x * px_to_rad; // left/right
                let pitch_step = -delta.y * px_to_rad; // up/down

                // 1) yaw around local up (east-west motion)
                let mut dir = up0;
                dir = rotate_dir(dir, up0, yaw_step);

                // 2) recompute frame at the new dir, then pitch along its local east
                let up1 = dir;
                let (east1, _) = tangent_frame(up1);
                dir = rotate_dir(dir, east1, pitch_step);

                cam.target_focus = dir * r;
            }

            // Smooth pose
            cam.focus = smooth_to_v(cam.focus, cam.target_focus, dt, TAU_POS);
            cam.altitude = smooth_to(cam.altitude, cam.target_altitude, dt, TAU_ZOOM);

            // Build view from local tangent frame
            let up = cam.focus.normalize();
            let (east, north) = tangent_frame(up);
            let forward = (Quat::from_axis_angle(up, cam.yaw)
                * Quat::from_axis_angle(east, cam.pitch))
                * (-north);
            let eye = cam.focus + up * cam.altitude.max(FP_ENTER_ALT);

            t.translation = eye;
            t.look_to(forward, up);
        }

        // ─────────────────────── FIRST PERSON ────────────────────────
        CameraMode::FirstPerson => {
            // RMB look
            if rmb {
                cam.yaw = wrap(cam.yaw - delta.x * ROT_SENS);
                cam.pitch = clamp_pitch(
                    cam.pitch - delta.y * ROT_SENS,
                    FP_PITCH_CLAMP.0,
                    FP_PITCH_CLAMP.1,
                );
            }

            // Walk: Z/S/Q/D (+ W/A/S/D aliases)
            let mut move_lr = 0.0; // left (-) / right (+)
            let mut move_fb = 0.0; // back (-) / forward (+)
            if keys.pressed(KeyCode::KeyZ) || keys.pressed(KeyCode::KeyW) {
                move_fb += 1.0;
            }
            if keys.pressed(KeyCode::KeyS) {
                move_fb -= 1.0;
            }
            if keys.pressed(KeyCode::KeyQ) || keys.pressed(KeyCode::KeyA) {
                move_lr -= 1.0;
            }
            if keys.pressed(KeyCode::KeyD) {
                move_lr += 1.0;
            }

            if move_lr != 0.0 || move_fb != 0.0 {
                let up = cam.target_focus.normalize();
                let (east, north) = tangent_frame(up);
                let fwd = (Quat::from_axis_angle(up, cam.yaw)
                    * Quat::from_axis_angle(east, cam.pitch))
                    * (-north);
                let fwd_t = (fwd - up * fwd.dot(up)).normalize_or_zero();
                let right = up.cross(fwd_t).normalize_or_zero();

                let mut wish = fwd_t * move_fb + right * move_lr;
                if wish.length_squared() > 0.0 {
                    wish = wish.normalize();
                }

                let speed = FP_SPEED
                    * if keys.pressed(KeyCode::ShiftLeft) {
                        2.0
                    } else {
                        1.0
                    };
                let step_rad = (speed * dt / r).clamp(0.0, 0.3);
                let axis = wish.cross(up).normalize_or_zero();
                let new_dir = rotate_dir(up, axis, step_rad);

                cam.target_focus = new_dir * r;
            }

            // Smooth pose
            cam.focus = smooth_to_v(cam.focus, cam.target_focus, dt, TAU_POS);
            cam.altitude = smooth_to(
                cam.altitude,
                FP_ENTER_ALT.max(cam.target_altitude),
                dt,
                TAU_ZOOM,
            );

            // View
            let up = cam.focus.normalize();
            let (east, north) = tangent_frame(up);
            let forward = (Quat::from_axis_angle(up, cam.yaw)
                * Quat::from_axis_angle(east, cam.pitch))
                * (-north);
            let eye = cam.focus + up * FP_ENTER_ALT;

            t.translation = eye;
            t.look_to(forward, up);
        }
    }
}
