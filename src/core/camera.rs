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
    pub yaw: f32,
    pub pitch: f32,
    pub radius: f32,
    pub focus: Vec3,
    pub last_cursor: Option<Vec2>,
}

fn despawn_existing_cameras(mut commands: Commands, cams: Query<Entity, With<Camera3d>>) {
    for e in &cams {
        commands.entity(e).despawn();
    }
}

fn spawn_editor_camera(mut commands: Commands) {
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

    // Orbit camera
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

#[inline]
fn orbit_to_eye(yaw: f32, pitch: f32, radius: f32, focus: Vec3) -> (Vec3, Vec3) {
    let dir = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
    let eye = focus - dir * radius.max(0.1);
    (eye, Vec3::Y)
}

fn update_editor_camera(
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>, // <-- fixed
    mut q: Query<(&mut Transform, &mut EditorCamera)>,
) {
    let dt = time.delta_secs();
    let Ok(window) = windows.single() else { return; };
    let cursor = window.cursor_position();

    let (mut t, mut cam) = match q.single_mut() {
        Ok(v) => v,
        Err(_) => return,
    };

    // rotate (RMB)
    if buttons.pressed(MouseButton::Right) {
        if let (Some(prev), Some(now)) = (cam.last_cursor, cursor) {
            let delta = now - prev;
            let sens = 0.01;
            cam.yaw -= delta.x * sens;
            cam.pitch -= delta.y * sens;
            cam.pitch = cam.pitch.clamp(-1.54, 1.54);
        }
    }

    // pan (MMB)
    if buttons.pressed(MouseButton::Middle) {
        if let (Some(prev), Some(now)) = (cam.last_cursor, cursor) {
            let delta = (now - prev) * 0.01 * cam.radius.max(1.0);
            let right = Quat::from_rotation_y(cam.yaw) * Vec3::X;
            let up = Vec3::Y;
            cam.focus -= right * delta.x;
            cam.focus += up * delta.y;
        }
    }

    cam.last_cursor = cursor;

    // zoom (wheel or Z/X)
    let mut zoom = 0.0f32;
    for evt in wheel.read() {
        zoom += match evt.unit {
            MouseScrollUnit::Line => evt.y * -1.0,
            MouseScrollUnit::Pixel => evt.y * -0.05,
        };
    }
    if keys.pressed(KeyCode::KeyZ) {
        zoom += -10.0 * dt;
    }
    if keys.pressed(KeyCode::KeyX) {
        zoom += 10.0 * dt;
    }
    cam.radius = (cam.radius * (1.0 + zoom * 0.1)).clamp(2.0, 500.0);

    // WASD/QE move
    let base = 10.0;
    let speed = if keys.pressed(KeyCode::ShiftLeft) {
        base * 4.0
    } else if keys.pressed(KeyCode::ControlLeft) {
        base * 0.25
    } else {
        base
    };

    let yaw_rot = Quat::from_rotation_y(cam.yaw);
    let forward = (yaw_rot * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let right = yaw_rot * Vec3::X;

    let mut move_vec = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        move_vec += forward;
    }
    if keys.pressed(KeyCode::KeyS) {
        move_vec -= forward;
    }
    if keys.pressed(KeyCode::KeyA) {
        move_vec -= right;
    }
    if keys.pressed(KeyCode::KeyD) {
        move_vec += right;
    }
    if keys.pressed(KeyCode::KeyQ) {
        move_vec.y -= 1.0;
    }
    if keys.pressed(KeyCode::KeyE) {
        move_vec.y += 1.0;
    }
    if move_vec.length_squared() > 0.0 {
        cam.focus += move_vec.normalize() * speed * dt;
    }

    // focus / reset
    if keys.just_pressed(KeyCode::KeyF) {
        cam.focus = Vec3::ZERO;
    }
    if keys.pressed(KeyCode::ShiftRight) && keys.just_pressed(KeyCode::KeyR) {
        cam.yaw = -0.65;
        cam.pitch = -0.45;
        cam.radius = 28.0;
        cam.focus = Vec3::ZERO;
    }

    let (eye, up) = orbit_to_eye(cam.yaw, cam.pitch, cam.radius, cam.focus);
    t.translation = eye;
    t.look_at(cam.focus, up);
}
