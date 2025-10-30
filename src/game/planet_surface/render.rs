//! Patch entity manager placeholder.
//!
//! The render layer will consume `stream::PatchAssetReady` events and spawn /
//! recycle entities with the appropriate meshes.  For now, we only provide type
//! definitions and no-op systems so other code can depend on them.

use std::collections::HashSet;

use bevy::prelude::*;

use super::{lod::PatchKey, stream::PatchRequestQueue};

#[derive(Component)]
pub struct SurfacePatch {
    pub key: PatchKey,
}

#[derive(Resource, Default)]
pub struct PatchStats {
    pub loaded: usize,
    pub requested: usize,
}

#[derive(Resource, Default)]
pub struct PatchRegistry {
    pub keys: HashSet<PatchKey>,
}

pub fn update_patch_stats(
    mut stats: ResMut<PatchStats>,
    patches: Query<&SurfacePatch>,
    queue: Res<PatchRequestQueue>,
    mut registry: ResMut<PatchRegistry>,
) {
    registry.keys.clear();
    for patch in patches.iter() {
        registry.keys.insert(patch.key);
    }
    stats.loaded = registry.keys.len();
    stats.requested = queue.len();
}
