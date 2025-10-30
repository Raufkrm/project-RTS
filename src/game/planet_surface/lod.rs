//! LOD descriptors for planet surface patches.
//!
//! The idea: represent the planet surface as a quadtree of patches indexed by
//! a `(face, x, y, level)` tuple (HEALPix, cube-sphere, etc.).  The current
//! stubs keep it generic so we can experiment before locking in a layout.

use bevy::math::Vec3;

/// Unique identifier for a surface patch in the quadtree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatchKey {
    pub face: u8,
    pub level: u8,
    pub ix: u32,
    pub iy: u32,
}

impl PatchKey {
    pub const ROOT_FACES: u8 = 6;

    pub fn parent(self) -> Option<Self> {
        if self.level == 0 {
            return None;
        }
        Some(Self {
            face: self.face,
            level: self.level - 1,
            ix: self.ix >> 1,
            iy: self.iy >> 1,
        })
    }

    pub fn children(self) -> [Self; 4] {
        let lvl = self.level + 1;
        let ix = self.ix << 1;
        let iy = self.iy << 1;
        [
            Self {
                face: self.face,
                level: lvl,
                ix,
                iy,
            },
            Self {
                face: self.face,
                level: lvl,
                ix: ix + 1,
                iy,
            },
            Self {
                face: self.face,
                level: lvl,
                ix,
                iy: iy + 1,
            },
            Self {
                face: self.face,
                level: lvl,
                ix: ix + 1,
                iy: iy + 1,
            },
        ]
    }
}

/// Minimal metadata needed to cull / morph a patch.
#[derive(Debug, Clone)]
pub struct PatchDescriptor {
    pub key: PatchKey,
    pub center: Vec3,
    pub radius: f32,
    pub max_height: f32,
    pub min_height: f32,
}

impl PatchDescriptor {
    pub fn screen_space_error(&self, camera_pos: Vec3, planet_radius: f32) -> f32 {
        // TODO: replace with geometric error metric once we have actual meshes.
        let dist = camera_pos.distance(self.center) - planet_radius;
        ((self.max_height - self.min_height).abs() + planet_radius * 0.001)
            / dist.max(planet_radius * 0.001)
    }
}
