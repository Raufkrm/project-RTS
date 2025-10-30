//! LOD descriptors for planet surface patches.
//!
//! The idea: represent the planet surface as a quadtree of patches indexed by
//! a `(face, x, y, level)` tuple (HEALPix, cube-sphere, etc.).  The current
//! stubs keep it generic so we can experiment before locking in a layout.

use bevy::math::{Vec3, Vec3A};

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
    pub fn from_key(key: PatchKey, planet_radius: f32) -> Self {
        let (u0, v0, u1, v1) = patch_uv_bounds(key);
        let corners = [
            cube_uv_to_dir(key.face, u0, v0),
            cube_uv_to_dir(key.face, u1, v0),
            cube_uv_to_dir(key.face, u0, v1),
            cube_uv_to_dir(key.face, u1, v1),
        ];

        let mut center_dir = Vec3::ZERO;
        for dir in corners.iter() {
            center_dir += *dir;
        }
        let center_dir = center_dir.normalize_or_zero();
        let center = center_dir * planet_radius;

        let mut radius: f32 = 0.0;
        for dir in corners.into_iter() {
            let pos = dir * planet_radius;
            radius = radius.max(pos.distance(center));
        }

        Self {
            key,
            center,
            radius,
            max_height: 0.0,
            min_height: 0.0,
        }
    }

    pub fn screen_space_error(&self, camera_pos: Vec3) -> f32 {
        let dist = camera_pos.distance(self.center).max(1.0);
        self.radius / dist
    }
}

pub fn patch_uv_bounds(key: PatchKey) -> (f32, f32, f32, f32) {
    let subdiv = 1u32 << key.level;
    let inv = 2.0 / subdiv as f32;
    let u0 = -1.0 + key.ix as f32 * inv;
    let v0 = -1.0 + key.iy as f32 * inv;
    let u1 = u0 + inv;
    let v1 = v0 + inv;
    (u0, v0, u1, v1)
}

pub fn cube_uv_to_dir(face: u8, u: f32, v: f32) -> Vec3 {
    let dir = match face {
        0 => Vec3A::new(1.0, v, -u),
        1 => Vec3A::new(-1.0, v, u),
        2 => Vec3A::new(u, 1.0, -v),
        3 => Vec3A::new(u, -1.0, v),
        4 => Vec3A::new(u, v, 1.0),
        5 => Vec3A::new(-u, v, -1.0),
        _ => Vec3A::Y,
    };
    dir.normalize_or_zero().into()
}
