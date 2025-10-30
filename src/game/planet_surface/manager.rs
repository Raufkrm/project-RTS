//! Planet context controller (orbit / approach / surface).
//!
//! Decides which layer should be active based on camera altitude and seeds
//! patch requests for nearer layers.  Actual streaming/rendering happens in the
//! sibling modules.

use bevy::prelude::*;

use super::{lod::PatchKey, stream::PatchRequestQueue};
use crate::core::galaxy_camera::MainCamera;
use crate::game::world::planet::{PlanetParams, PlanetTag};

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
    root_patches_requested: bool,
}

impl Default for PlanetContext {
    fn default() -> Self {
        Self {
            layer: PlanetContextLayer::Orbit,
            altitude_km: 0.0,
            orbit_to_approach_km: 120.0,
            approach_to_surface_km: 12.0,
            root_patches_requested: false,
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
) {
    let cam_tf = match transforms.p0().iter().next() {
        Some(tf) => *tf,
        None => return,
    };
    let planet_tf = match transforms.p1().iter().next() {
        Some(tf) => *tf,
        None => return,
    };

    let center = planet_tf.translation();
    let dist = cam_tf.translation().distance(center);
    let radius = params.radius.max(1.0);
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
        context.root_patches_requested = false;
    }

    context.altitude_km = altitude_km;

    // We only request root patches once we enter the approach layer (and below).
    if matches!(
        context.layer,
        PlanetContextLayer::Approach | PlanetContextLayer::Surface
    ) && !context.root_patches_requested
    {
        for face in 0..PatchKey::ROOT_FACES {
            let key = PatchKey {
                face,
                level: 0,
                ix: 0,
                iy: 0,
            };
            queue.enqueue(key);
        }
        context.root_patches_requested = true;
        info!(
            "Queued root patches for streaming (queue_len={})",
            queue.len()
        );
    }
}
