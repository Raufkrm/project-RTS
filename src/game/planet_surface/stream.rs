//! Streaming queue scaffolding.
//!
//! The real streamer will run async tasks to load patch assets.  At the moment
//! it only keeps track of requested keys so we can start wiring it into the
//! planet systems without blocking on asset formats.

use std::collections::{HashSet, VecDeque};

use bevy::prelude::*;

use super::lod::PatchKey;

#[derive(Resource, Default)]
pub struct PatchRequestQueue {
    /// Keys awaiting load.
    pending: VecDeque<PatchKey>,
    /// Fast lookup to avoid duplicate enqueues.
    in_queue: HashSet<PatchKey>,
}

impl PatchRequestQueue {
    pub fn enqueue(&mut self, key: PatchKey) {
        if self.in_queue.insert(key) {
            self.pending.push_back(key);
        }
    }

    pub fn pop(&mut self) -> Option<PatchKey> {
        let key = self.pending.pop_front();
        if let Some(k) = key {
            self.in_queue.remove(&k);
        }
        key
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
    // TODO: bridge to async loader. We leave the system in place so the schedule
    // can include it without panicking.
    while queue.len() > 512 {
        if let Some(key) = queue.pending.pop_back() {
            queue.in_queue.remove(&key);
        } else {
            break;
        }
    }
}
