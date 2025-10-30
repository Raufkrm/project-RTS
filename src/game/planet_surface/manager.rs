//! Planet context controller (orbit / approach / surface).
//!
//! Decides which layer should be active based on camera altitude and gathers
//! patch requests for the terrain streamer. Actual streaming/rendering happens
//! in sibling modules.

use std::collections::{HashSet, VecDeque};

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
const BACKFACE_CUTOFF: f32 = -0.2;

#[derive(Resource, Clone, Copy, Debug)]
pub struct PlanetLodConfig {
    pub surface_error: f32,
    pub approach_error: f32,
    pub max_surface_level: u8,
    pub max_approach_level: u8,
}

impl Default for PlanetLodConfig {
    fn default() -> Self {
        Self {
            surface_error: DEFAULT_SURFACE_ERROR_THRESHOLD,
            approach_error: DEFAULT_APPROACH_ERROR_THRESHOLD,
            max_surface_level: DEFAULT_MAX_SURFACE_LEVEL,
            max_approach_level: DEFAULT_MAX_APPROACH_LEVEL,
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
}

impl Default for PlanetContext {
    fn default() -> Self {
        Self {
            layer: PlanetContextLayer::Orbit,
            altitude_km: 0.0,
            orbit_to_approach_km: 120.0,
            approach_to_surface_km: 12.0,
            desired: HashSet::default(),
        }
    }
}

pub fn update_planet_context(
    params: Res<PlanetParams>,
    mut context: ResMut<PlanetContext>,
    mut transforms: ParamSet<(
        Query<&GlobalTransform, With<MainCamera>>,
        Query<&GlobalTransform, With<PlanetTag>>,
    )>,
    mut queue: ResMut<PatchRequestQueue>,
    lod: Res<PlanetLodConfig>,
) {
    let cam_tf = match transforms.p0().iter().next() {
        Some(tf) => *tf,
        None => return,
    };
    let planet_tf = match transforms.p1().iter().next() {
        Some(tf) => *tf,
        None => return,
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
        queue.clear();
    }

    context.altitude_km = altitude_km;

    if !matches!(context.layer, PlanetContextLayer::Approach | PlanetContextLayer::Surface) {
        queue.clear();
        context.desired.clear();
        return;
    }

    let new_desired = compute_desired_patches(
        context.layer,
        radius,
        planet_center,
        camera_pos,
        altitude,
        &*lod,
    );

    queue.retain_keys(&new_desired);

    for key in new_desired.iter().copied() {
        if !context.desired.contains(&key) {
            queue.enqueue(key);
        }
    }

    context.desired = new_desired;
}

fn compute_desired_patches(
    layer: PlanetContextLayer,
    radius: f32,
    planet_center: Vec3,
    camera_pos: Vec3,
    altitude: f32,
    lod: &PlanetLodConfig,
) -> HashSet<PatchKey> {
    let mut desired = HashSet::default();
    if !matches!(layer, PlanetContextLayer::Approach | PlanetContextLayer::Surface) {
        return desired;
    }

    let max_level = match layer {
        PlanetContextLayer::Approach => lod.max_approach_level,
        PlanetContextLayer::Surface => lod.max_surface_level,
        PlanetContextLayer::Orbit => 0,
    };

    let to_camera = camera_pos - planet_center;
    let to_camera_dir = to_camera.normalize_or_zero();
    let include_full_sphere = to_camera_dir.length_squared() < f32::EPSILON;

    let altitude_ratio = (altitude / (radius * 4.0)).clamp(0.0, 1.0);
    let threshold = lod.surface_error
        + (lod.approach_error - lod.surface_error) * altitude_ratio;

    let mut stack = VecDeque::new();
    for face in 0..PatchKey::ROOT_FACES {
        stack.push_back(PatchKey {
            face,
            level: 0,
            ix: 0,
            iy: 0,
        });
    }

    while let Some(key) = stack.pop_front() {
        let descriptor = PatchDescriptor::from_key(key, radius);
        let center_local = descriptor.center;
        let to_patch = center_local.normalize_or_zero();

        if !include_full_sphere && to_patch.dot(to_camera_dir) < BACKFACE_CUTOFF {
            continue;
        }

        let center_world = planet_center + center_local;
        let dist = camera_pos.distance(center_world).max(1.0);
        let angular_error = descriptor.radius / dist;

        if key.level < max_level && angular_error > threshold {
            for child in key.children() {
                stack.push_back(child);
            }
        } else {
            desired.insert(key);
        }
    }

    desired
}
