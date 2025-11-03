//! Planet context controller (orbit / approach / surface).
//!
//! Decides which layer should be active based on camera altitude and gathers
//! patch requests for the terrain streamer. Actual streaming/rendering happens
//! in sibling modules.

use std::collections::{HashSet, VecDeque};

use bevy::camera::primitives::Frustum;
use bevy::prelude::*;

use super::{
    lod::{PatchDescriptor, PatchKey},
    stream::PatchRequestQueue,
};
use crate::core::galaxy_camera::MainCamera;
use crate::game::world::planet::{PlanetParams, PlanetTag};

pub const DEFAULT_MAX_APPROACH_LEVEL: u8 = 3;
pub const DEFAULT_MAX_SURFACE_LEVEL: u8 = 6;
pub const DEFAULT_SURFACE_ERROR_THRESHOLD: f32 = 0.02;
pub const DEFAULT_APPROACH_ERROR_THRESHOLD: f32 = 0.14;
pub const DEFAULT_SURFACE_FALLOFF_KM: f32 = 25.0;
pub const DEFAULT_APPROACH_FALLOFF_KM: f32 = 160.0;
pub const DEFAULT_SURFACE_MIN_LEVEL: u8 = 2;
const BACKFACE_CUTOFF: f32 = -0.2;
const SURFACE_VIEW_BASE_DEG: f32 = 35.0;
const SURFACE_VIEW_MAX_DEG: f32 = 60.0;
const SURFACE_VIEW_ALT_KM: f32 = 20.0;
const APPROACH_VIEW_BASE_DEG: f32 = 70.0;
const APPROACH_VIEW_MAX_DEG: f32 = 105.0;
const APPROACH_VIEW_ALT_KM: f32 = 180.0;

#[derive(Resource, Clone, Copy, Debug)]
pub struct PlanetLodConfig {
    pub surface_error: f32,
    pub approach_error: f32,
    pub max_surface_level: u8,
    pub max_approach_level: u8,
    pub surface_falloff_km: f32,
    pub approach_falloff_km: f32,
    pub surface_min_level: u8,
}

impl Default for PlanetLodConfig {
    fn default() -> Self {
        Self {
            surface_error: DEFAULT_SURFACE_ERROR_THRESHOLD,
            approach_error: DEFAULT_APPROACH_ERROR_THRESHOLD,
            max_surface_level: DEFAULT_MAX_SURFACE_LEVEL,
            max_approach_level: DEFAULT_MAX_APPROACH_LEVEL,
            surface_falloff_km: DEFAULT_SURFACE_FALLOFF_KM,
            approach_falloff_km: DEFAULT_APPROACH_FALLOFF_KM,
            surface_min_level: DEFAULT_SURFACE_MIN_LEVEL,
        }
    }
}

/// High-level context around the planet camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetContextLayer {
    Orbit,
    Approach,
    Surface,
}

#[derive(Resource, Debug)]
pub struct PlanetContext {
    pub layer: PlanetContextLayer,
    pub altitude_km: f32,
    pub orbit_to_approach_km: f32,
    pub approach_to_surface_km: f32,
    pub desired: HashSet<PatchKey>,
    pub max_requested_level: u8,
    pub camera_pos: Vec3,
    pub planet_center: Vec3,
    pub surface_morph: f32,
}

impl Default for PlanetContext {
    fn default() -> Self {
        Self {
            layer: PlanetContextLayer::Orbit,
            altitude_km: 0.0,
            orbit_to_approach_km: 120.0,
            approach_to_surface_km: 12.0,
            desired: HashSet::default(),
            max_requested_level: 0,
            camera_pos: Vec3::ZERO,
            planet_center: Vec3::ZERO,
            surface_morph: 0.0,
        }
    }
}

pub fn update_planet_context(
    params: Res<PlanetParams>,
    mut context: ResMut<PlanetContext>,
    mut transforms: ParamSet<(
        Query<(&GlobalTransform, Option<&Frustum>), With<MainCamera>>,
        Query<&GlobalTransform, With<PlanetTag>>,
    )>,
    mut queue: ResMut<PatchRequestQueue>,
    lod: Res<PlanetLodConfig>,
) {
    let (cam_tf, camera_frustum_opt) = {
        let camera_query = transforms.p0();
        match camera_query.iter().next() {
            Some((tf, frustum)) => (*tf, frustum.copied()),
            None => return,
        }
    };

    let planet_tf = {
        let planet_query = transforms.p1();
        match planet_query.iter().next() {
            Some(tf) => *tf,
            None => return,
        }
    };

    let camera_pos = cam_tf.translation();
    let planet_center = planet_tf.translation();
    let radius = params.radius.max(1.0);
    let dist = camera_pos.distance(planet_center);
    let altitude = (dist - radius).max(0.0);
    let altitude_km = altitude * 0.001;

    let layer = match context.layer {
        PlanetContextLayer::Orbit => {
            if altitude_km <= context.orbit_to_approach_km {
                PlanetContextLayer::Approach
            } else {
                PlanetContextLayer::Orbit
            }
        }
        PlanetContextLayer::Approach => {
            if altitude_km <= context.approach_to_surface_km {
                PlanetContextLayer::Surface
            } else if altitude_km > context.orbit_to_approach_km * 1.1 {
                // hysteresis to avoid jitter bouncing.
                PlanetContextLayer::Orbit
            } else {
                PlanetContextLayer::Approach
            }
        }
        PlanetContextLayer::Surface => {
            if altitude_km > context.approach_to_surface_km * 1.2 {
                PlanetContextLayer::Approach
            } else {
                PlanetContextLayer::Surface
            }
        }
    };

    if layer != context.layer {
        info!(
            "Planet context switched {:?} -> {:?} (alt {:.1} km)",
            context.layer, layer, altitude_km
        );
        context.layer = layer;
        context.desired.clear();
        context.max_requested_level = 0;
        queue.clear();
    }

    context.altitude_km = altitude_km;
    context.camera_pos = camera_pos;
    context.planet_center = planet_center;
    context.surface_morph = match context.layer {
        PlanetContextLayer::Surface => 1.0,
        PlanetContextLayer::Approach => {
            let falloff = lod.surface_falloff_km.max(1.0);
            let delta = (altitude_km - context.approach_to_surface_km).max(0.0);
            let t = 1.0 - (delta / falloff).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        }
        PlanetContextLayer::Orbit => 0.0,
    };

    if !matches!(
        context.layer,
        PlanetContextLayer::Approach | PlanetContextLayer::Surface
    ) {
        queue.clear();
        context.desired.clear();
        context.max_requested_level = 0;
        return;
    }

    let (mut new_desired, max_requested_level) = compute_desired_patches(
        context.layer,
        radius,
        planet_center,
        camera_pos,
        altitude,
        &*lod,
        camera_frustum_opt.as_ref(),
    );

    queue.retain_keys(&new_desired);

    let mut newly_requested = 0usize;
    let max_new_per_frame = match context.layer {
        PlanetContextLayer::Surface => 40,
        PlanetContextLayer::Approach => 24,
        PlanetContextLayer::Orbit => 0,
    };

    for key in new_desired.clone() {
        if !context.desired.contains(&key) {
            if newly_requested < max_new_per_frame {
                queue.enqueue(key);
                newly_requested += 1;
            } else {
                new_desired.remove(&key);
            }
        }
    }

    context.desired = new_desired;
    context.max_requested_level = max_requested_level;
}

fn compute_desired_patches(
    layer: PlanetContextLayer,
    radius: f32,
    planet_center: Vec3,
    camera_pos: Vec3,
    altitude: f32,
    lod: &PlanetLodConfig,
    camera_frustum: Option<&Frustum>,
) -> (HashSet<PatchKey>, u8) {
    let mut desired = HashSet::default();
    if !matches!(
        layer,
        PlanetContextLayer::Approach | PlanetContextLayer::Surface
    ) {
        return (desired, 0);
    }

    let mut max_level = match layer {
        PlanetContextLayer::Approach => lod.max_approach_level,
        PlanetContextLayer::Surface => lod.max_surface_level,
        PlanetContextLayer::Orbit => 0,
    };

    if matches!(layer, PlanetContextLayer::Surface) {
        let falloff_m = (lod.surface_falloff_km.max(0.5) * 1000.0).max(10.0);
        let min_level = lod.surface_min_level.min(lod.max_surface_level);
        let range = max_level.saturating_sub(min_level);
        if range > 0 {
            let ratio = (altitude / falloff_m).clamp(0.0, 1.0);
            let reduction = ((range as f32) * ratio.powf(1.2)).floor() as u8;
            max_level = max_level.saturating_sub(reduction).max(min_level);
        } else {
            max_level = max_level.max(min_level);
        }
    } else if matches!(layer, PlanetContextLayer::Approach) {
        let falloff_m = (lod.approach_falloff_km.max(1.0) * 1000.0).max(50.0);
        let ratio = (altitude / falloff_m).clamp(0.0, 1.0);
        let reduction = ((max_level as f32) * ratio.powf(1.1)).floor() as u8;
        max_level = max_level.saturating_sub(reduction);
    }

    let to_camera = camera_pos - planet_center;
    let to_camera_dir = to_camera.normalize_or_zero();
    let include_full_sphere = to_camera_dir.length_squared() < f32::EPSILON;

    let altitude_ratio = (altitude / (radius * 4.0)).clamp(0.0, 1.0);
    let threshold = lod.surface_error + (lod.approach_error - lod.surface_error) * altitude_ratio;

    let mut stack = VecDeque::new();
    let mut visited: HashSet<PatchKey> = HashSet::default();
    for face in 0..PatchKey::ROOT_FACES {
        stack.push_back(PatchKey {
            face,
            level: 0,
            ix: 0,
            iy: 0,
        });
    }

    let mut deepest = 0u8;

    while let Some(key) = stack.pop_front() {
        if !visited.insert(key) {
            continue;
        }
        let descriptor = PatchDescriptor::from_key(key, radius);
        let center_local = descriptor.center;
        let to_patch = center_local.normalize_or_zero();
        let center_world = planet_center + center_local;
        let dist = camera_pos.distance(center_world).max(1.0);

        if let Some(frustum) = camera_frustum {
            if sphere_outside_frustum(frustum, center_world, descriptor.radius) {
                continue;
            }
        }

        if !include_full_sphere {
            let view_dot = to_patch.dot(to_camera_dir);
            let focus = (altitude / (radius * 2.5)).clamp(0.0, 1.0);
            let front_cutoff_base = BACKFACE_CUTOFF + focus * 0.65;
            let patch_angle = (descriptor.radius / (radius + altitude)).clamp(0.0, 0.5);
            let low_alt_boost = (1.0 - (altitude / (radius * 0.75)).clamp(0.0, 1.0)) * 0.1;
            let horizon_margin = (patch_angle * 1.35 + low_alt_boost).clamp(0.0, 0.55);
            let front_cutoff = (front_cutoff_base - horizon_margin).max(-0.95);
            if view_dot < front_cutoff {
                continue;
            }

            let altitude_km = altitude * 0.001;
            let max_angle_deg = match layer {
                PlanetContextLayer::Surface => {
                    let t = (altitude_km / SURFACE_VIEW_ALT_KM).clamp(0.0, 1.0);
                    SURFACE_VIEW_BASE_DEG + (SURFACE_VIEW_MAX_DEG - SURFACE_VIEW_BASE_DEG) * t
                }
                PlanetContextLayer::Approach => {
                    let t = (altitude_km / APPROACH_VIEW_ALT_KM).clamp(0.0, 1.0);
                    APPROACH_VIEW_BASE_DEG + (APPROACH_VIEW_MAX_DEG - APPROACH_VIEW_BASE_DEG) * t
                }
                PlanetContextLayer::Orbit => 180.0,
            };
            let max_angle_rad = max_angle_deg.to_radians();
            let patch_half_angle = (descriptor.radius / dist).clamp(0.0, 1.2);
            let view_angle = view_dot.clamp(-1.0, 1.0).acos();
            if view_angle > max_angle_rad + patch_half_angle {
                continue;
            }
        }

        let angular_error = descriptor.radius / dist;

        deepest = deepest.max(key.level);

        let level_falloff = (1.0 - key.level as f32 * 0.05).max(0.35);
        let adaptive_threshold = threshold * level_falloff;

        desired.insert(key);

        if key.level < max_level && angular_error > adaptive_threshold {
            for child in key.children() {
                stack.push_back(child);
            }
            continue;
        }
    }

    (desired, deepest)
}

fn sphere_outside_frustum(frustum: &Frustum, center: Vec3, radius: f32) -> bool {
    for half_space in &frustum.half_spaces {
        let normal: Vec3 = half_space.normal().into();
        if normal.dot(center) + half_space.d() + radius < 0.0 {
            return true;
        }
    }
    false
}
