use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::app::AppState;

pub struct EditorCameraPlugin;
impl Plugin for EditorCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnEnter(AppState::InGame),
            (despawn_existing_cameras, spawn_editor_camera),
        )
        .add_systems(OnExit(AppState::InGame), despawn_editor_camera)
        .add_systems(Update, update_editor_camera.run_if(in_state(AppState::InGame)));
    }
}

#[derive(Component)]
pub struct EditorCamera {
    // current (rendered) state
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    pub focus: Vec3,

    // targets for smooth damping
    pub target_yaw: f32,
    pub target_pitch: f32,
    pub target_radius: f32,
    pub target_focus: Vec3,

    pub last_cursor: Option<Vec2>,
}

// --- lifecycle ---
fn despawn_existing_cameras(mut commands: Commands, cams: Query<Entity, With<Camera3d>>) {
    for e in &cams {
        commands.entity(e).despawn();
    }
}

fn spawn_editor_camera(mut commands: Commands) {
    // Sun for shading inspection
    commands.spawn((
        DirectionalLight {
            shadows_enabled: true,
            illuminance: 25_000.0,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.9, 0.7, 0.0)),
        Name::new("Sun"),
    ));

    // Default orbit setup
    let yaw = -0.65;
    let pitch = -0.45;
    let radius = 28.0;
    let focus = Vec3::ZERO;
    let (eye, up) = orbit_to_eye(yaw, pitch, radius, focus);

    commands.spawn((
        Camera3d::default(),
        Transform::from_translation(eye).looking_at(focus, up),
        EditorCamera {
            yaw,
            pitch,
            radius,
            focus,
            target_yaw: yaw,
            target_pitch: pitch,
            target_radius: radius,
            target_focus: focus,
            last_cursor: None,
        },
        Name::new("EditorCamera"),
    ));
}

fn despawn_editor_camera(
    mut commands: Commands,
    q_cam: Query<Entity, With<EditorCamera>>,
    q_light: Query<Entity, With<DirectionalLight>>,
) {
    for e in q_cam.iter().chain(q_light.iter()) {
        commands.entity(e).despawn();
    }
}

// --- math helpers ---
#[inline]
fn orbit_to_eye(yaw: f32, pitch: f32, radius: f32, focus: Vec3) -> (Vec3, Vec3) {
    let dir = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
    let eye = focus - dir * radius.max(0.0001);
    (eye, Vec3::Y)
}

// critically-damped smoothing: new = lerp(current, target, 1 - e^{-dt/tau})
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

// --- controls ---
fn update_editor_camera(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut q: Query<(&mut Transform, &mut EditorCamera)>,
) {
    let dt = time.delta_secs();
    let Ok(window) = windows.single() else { return; };
    let cursor = window.cursor_position();

    let (mut t, mut cam) = match q.single_mut() {
        Ok(v) => v,
        Err(_) => return,
    };

    // -------- Input collection --------

    // Orbit (RMB)
    if buttons.pressed(MouseButton::Right) {
        if let (Some(prev), Some(now)) = (cam.last_cursor, cursor) {
            let delta = now - prev;
            let sens = 0.01;
            cam.target_yaw   -= delta.x * sens;
            cam.target_pitch -= delta.y * sens;
            cam.target_pitch = cam.target_pitch.clamp(-1.54, 1.54);
        }
    }

    // Pan (MMB) — scale with distance
    if buttons.pressed(MouseButton::Middle) {
        if let (Some(prev), Some(now)) = (cam.last_cursor, cursor) {
            let delta = (now - prev) * 0.01 * cam.target_radius.max(0.01);
            let right = Quat::from_rotation_y(cam.target_yaw) * Vec3::X;
            let up = Vec3::Y;
            cam.target_focus -= right * delta.x;
            cam.target_focus += up * delta.y;
        }
    }

    cam.last_cursor = cursor;

    // Zoom (wheel or Z/X). Use multiplicative scaling for true continuous, clamp-free zoom.
    // No max radius; only tiny epsilon min to avoid flipping through focus.
    let mut zoom_steps = 0.0f32;
    for evt in wheel.read() {
        zoom_steps += match evt.unit {
            MouseScrollUnit::Line => evt.y * -1.0,     // invert if desired
            MouseScrollUnit::Pixel => evt.y * -0.05,
        };
    }
    if keys.pressed(KeyCode::KeyZ) { zoom_steps += -10.0 * dt; }
    if keys.pressed(KeyCode::KeyX) { zoom_steps +=  10.0 * dt; }

    if zoom_steps != 0.0 {
        // exponential zoom factor; small steps near surface, bigger far away
        // factor < 1 => zoom in, factor > 1 => zoom out
        let factor = (1.0 + zoom_steps * 0.15).max(0.01);
        cam.target_radius = (cam.target_radius * factor).max(0.001);
    }

    // WASD/QE free move (moves the focus). Speed scales with radius for consistent feel.
    let mut move_vec = Vec3::ZERO;
    let yaw_rot = Quat::from_rotation_y(cam.target_yaw);
    let forward = (yaw_rot * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let right = yaw_rot * Vec3::X;

    if keys.pressed(KeyCode::KeyW) { move_vec += forward; }
    if keys.pressed(KeyCode::KeyS) { move_vec -= forward; }
    if keys.pressed(KeyCode::KeyA) { move_vec -= right; }
    if keys.pressed(KeyCode::KeyD) { move_vec += right; }
    if keys.pressed(KeyCode::KeyQ) { move_vec.y -= 1.0; }
    if keys.pressed(KeyCode::KeyE) { move_vec.y += 1.0; }

    if move_vec.length_squared() > 0.0 {
        let base = 0.6; // tune feel
        // scale with distance, but give a little minimum for close-up work
        let speed = (cam.target_radius * base).max(2.0);
        let speed = if keys.pressed(KeyCode::ShiftLeft) {
            speed * 4.0
        } else if keys.pressed(KeyCode::ControlLeft) {
            speed * 0.25
        } else { speed };
        cam.target_focus += move_vec.normalize() * speed * dt;
    }

    // Focus / reset shortcuts
    if keys.just_pressed(KeyCode::KeyF) {
        cam.target_focus = Vec3::ZERO;
    }
    if keys.pressed(KeyCode::ShiftRight) && keys.just_pressed(KeyCode::KeyR) {
        cam.target_yaw = -0.65;
        cam.target_pitch = -0.45;
        cam.target_radius = 28.0;
        cam.target_focus = Vec3::ZERO;
    }

    // -------- Smooth damping to targets (no snapping) --------
    // Tau values are "time-to ~63% there". Smaller = snappier.
    let tau_rot = 0.06;
    let tau_zoom = 0.08;
    let tau_pan  = 0.06;

    cam.yaw    = smooth_to(cam.yaw,    cam.target_yaw,    dt, tau_rot);
    cam.pitch  = smooth_to(cam.pitch,  cam.target_pitch,  dt, tau_rot);
    cam.radius = smooth_to(cam.radius, cam.target_radius, dt, tau_zoom).max(0.001);
    cam.focus  = smooth_to_v(cam.focus, cam.target_focus, dt, tau_pan);

    // -------- Apply to transform --------
    let (eye, up) = orbit_to_eye(cam.yaw, cam.pitch, cam.radius, cam.focus);
    t.translation = eye;
    t.look_at(cam.focus, up);
}
