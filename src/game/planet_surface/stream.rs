//! Streaming queue scaffolding.
//!
//! The real streamer will run async tasks to load patch assets.  At the moment
//! it only keeps track of requested keys so we can start wiring it into the
//! planet systems without blocking on asset formats.

use std::collections::VecDeque;

use bevy::prelude::*;

use super::lod::PatchKey;

#[derive(Resource, Default)]
pub struct PatchRequestQueue {
    /// Keys awaiting load.
    pending: VecDeque<PatchKey>,
}

impl PatchRequestQueue {
    pub fn enqueue(&mut self, key: PatchKey) {
        if !self.pending.contains(&key) {
            self.pending.push_back(key);
        }
    }

    pub fn pop(&mut self) -> Option<PatchKey> {
        self.pending.pop_front()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }
}

pub fn drain_requests_system(mut queue: ResMut<PatchRequestQueue>) {
    // TODO: bridge to async loader. We leave the system in place so the schedule
    // can include it without panicking.
    if queue.len() > 512 {
        queue.pending.truncate(512);
    }
}
