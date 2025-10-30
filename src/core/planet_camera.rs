use bevy::prelude::*;

use crate::game::world::planet::{PlanetParams, PlanetTag};

/// Opt-in tag: add this to your camera to get altitude-scaled FOV.
#[derive(Component)]
pub struct ScaleFovByAltitude {
    /// FOV when hugging the surface (degrees)
    pub near_deg: f32,
    /// FOV in deep orbit (degrees)
    pub far_deg: f32,
    /// How many planet radii until we reach far_deg
    pub far_at_radii: f32,
}

/// Opt-in tag: add this to also scale mouse-wheel zoom by altitude.
#[derive(Component)]
pub struct ScaledWheelZoom {
    /// Base zoom speed in world units per scroll “line”
    pub base_speed: f32,
    /// Extra speed per planet radius of altitude
    pub speed_per_radius: f32,
    /// 0..1 smoothing per frame (0 = instant, 1 = frozen)
    pub smoothing: f32,
}

pub struct PlanetScaleCameraPlugin;
impl Plugin for PlanetScaleCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (adaptive_fov_by_altitude, altitude_scaled_wheel_zoom),
        );
    }
}

fn adaptive_fov_by_altitude(
    params: Res<PlanetParams>,
    q_planet: Query<&GlobalTransform, With<PlanetTag>>,
    mut q_cam: Query<(&GlobalTransform, &mut Projection, &ScaleFovByAltitude), With<Camera3d>>,
) {
    let Some(center_tf) = q_planet.iter().next() else {
        return;
    };
    let center = center_tf.translation();
    for (cam_gtf, mut proj, cfg) in &mut q_cam {
        let dist = cam_gtf.translation().distance(center);
        let alt = (dist - params.radius).max(0.0);
        let radii = alt / params.radius.max(1.0);
        let t = (radii / cfg.far_at_radii.max(0.001)).clamp(0.0, 1.0);

        let near = cfg.near_deg.to_radians();
        let far = cfg.far_deg.to_radians();
        let target = near + (far - near) * t;

        if let Projection::Perspective(p) = &mut *proj {
            // tiny smoothing to avoid popping
            p.fov = p.fov + (target - p.fov) * 0.15;
        }
    }
}

fn altitude_scaled_wheel_zoom(
    time: Res<Time>,
    params: Res<PlanetParams>,
    q_planet: Query<&GlobalTransform, With<PlanetTag>>,
    mut q_cam: Query<
        (&mut Transform, &GlobalTransform, &ScaledWheelZoom),
        (With<Camera3d>, Without<PlanetTag>),
    >,
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
) {
    let Some(center_tf) = q_planet.iter().next() else {
        return;
    };
    let center = center_tf.translation();

    // accumulate scroll across all events this frame, normalized to "lines"
    let mut scroll_lines = 0.0f32;
    for e in wheel.read() {
        let delta = match e.unit {
            bevy::input::mouse::MouseScrollUnit::Line => e.y.signum(),
            bevy::input::mouse::MouseScrollUnit::Pixel => (e.y / 120.0).clamp(-1.0, 1.0),
        } as f32;
        scroll_lines += delta;
    }
    if scroll_lines.abs() < f32::EPSILON {
        return;
    }
    scroll_lines = scroll_lines.clamp(-2.0, 2.0);

    for (mut t, g, cfg) in &mut q_cam {
        let dist = g.translation().distance(center);
        let alt = (dist - params.radius).max(0.0);
        let radii = alt / params.radius.max(1.0);

        // Speed grows with altitude (fast in space, precise near ground)
        let target_speed = cfg.base_speed * (1.0 + radii * cfg.speed_per_radius);
        // Smooth it a touch so big altitude swings don’t spike
        let s = cfg.smoothing.clamp(0.0, 0.95);
        let dt = time.delta_secs();
        let lerp = 1.0 - s.powf(dt * 60.0); // frame-rate independent

        // Zoom direction = camera forward
        let fwd = g.forward(); // Bevy 0.18 provides .forward() on transforms
        let step = (t.translation + fwd * scroll_lines * target_speed * dt)
            .lerp(t.translation, 1.0 - lerp);
        t.translation = step;
    }
}
