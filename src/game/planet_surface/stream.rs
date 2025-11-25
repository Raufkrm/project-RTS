//! Streaming queue scaffolding.
//!
//! The real streamer will run async tasks to load patch assets.  At the moment
//! it only keeps track of requested keys so we can start wiring it into the
//! planet systems without blocking on asset formats.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::Task;
use bevy_mesh::Mesh;

use super::lod::PatchKey;

#[derive(Resource, Default)]
pub struct PatchRequestQueue {
    /// Keys awaiting load.
    pending: Vec<PatchKey>,
    /// Fast lookup to avoid duplicate enqueues.
    in_queue: HashSet<PatchKey>,
}

impl PatchRequestQueue {
    pub fn enqueue(&mut self, key: PatchKey) {
        if self.in_queue.insert(key) {
            self.pending.push(key);
        }
    }

    pub fn pop_best<F>(&mut self, mut priority_fn: F) -> Option<(PatchKey, f32)>
    where
        F: FnMut(PatchKey) -> f32,
    {
        if self.pending.is_empty() {
            return None;
        }
        let mut best_index = 0usize;
        let mut best_priority = priority_fn(self.pending[0]);
        for (index, key) in self.pending.iter().copied().enumerate().skip(1) {
            let priority = priority_fn(key);
            if priority < best_priority {
                best_index = index;
                best_priority = priority;
            }
        }
        let removed = self.pending.swap_remove(best_index);
        self.in_queue.remove(&removed);
        Some((removed, best_priority))
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn retain_keys(&mut self, allowed: &HashSet<PatchKey>) {
        self.pending.retain(|key| {
            if allowed.contains(key) {
                true
            } else {
                self.in_queue.remove(key);
                false
            }
        });
    }

    pub fn clear(&mut self) {
        self.pending.clear();
        self.in_queue.clear();
    }
}

pub fn drain_requests_system(mut queue: ResMut<PatchRequestQueue>) {
    while queue.len() > 256 {
        if queue.pop_best(|_| 0.0f32).is_none() {
            break;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PatchLoadMethod {
    Disk,
    Procedural,
}

pub struct PatchLoadTask {
    pub method: PatchLoadMethod,
    pub task: Task<Option<GeneratedPatch>>,
}

#[derive(Resource, Default)]
pub struct PatchLoadTasks {
    pub tasks: HashMap<PatchKey, PatchLoadTask>,
}

#[derive(Clone, Debug)]
pub struct PatchPropPlaceholder {
    pub kind: String,
    pub position: Vec3,
    pub rotation_euler: Vec3,
    pub scale: Vec3,
}

pub struct GeneratedPatch {
    pub mesh: Mesh,
    pub samples: u32,
    pub build_time: Duration,
    pub sphere_positions: Vec<Vec3>,
    pub displaced_positions: Vec<Vec3>,
    pub sphere_normals: Vec<Vec3>,
    pub displaced_normals: Vec<Vec3>,
    pub materials: Vec<String>,
    pub props: Vec<PatchPropPlaceholder>,
    pub albedo_path: Option<String>,
    pub normal_path: Option<String>,
    pub roughness_path: Option<String>,
}
