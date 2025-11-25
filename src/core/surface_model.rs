use bevy::prelude::*;
use crate::game::world::surface_grid::SurfaceGrid;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceCoord {
    pub face: u8,  // 0..5 cube face index
    pub uv: Vec2,  // [0,1]x[0,1] on that face
    pub height: f32, // meters above base radius
}

#[derive(Resource, Clone)]
pub struct PlanetSurfaceModel {
    pub radius: f32,                 // base sphere radius in meters
    pub biome_resolution: u32,       // 1024 for now
    pub biome_faces: [Handle<Image>; 6], // cube-face biome textures
}

impl Default for PlanetSurfaceModel {
    fn default() -> Self {
        Self {
            radius: 10_000.0,
            biome_resolution: 1024,
            biome_faces: std::array::from_fn(|_| Handle::default()),
        }
    }
}

impl PlanetSurfaceModel {
    pub fn surface_to_world(&self, coord: SurfaceCoord) -> Vec3 {
        let dir = cube_face_dir(coord.face, coord.uv).normalize_or_zero();
        dir * (self.radius + coord.height)
    }

    pub fn world_to_surface(&self, world: Vec3) -> SurfaceCoord {
        let len = world.length();
        let dir = if len <= f32::EPSILON {
            Vec3::Z
        } else {
            world / len
        };

        let (face, uv) = dir_to_face_uv(dir);
        SurfaceCoord {
            face,
            uv,
            height: len - self.radius,
        }
    }

    /// Sample height (meters above base radius) along a direction using the surface grid.
    /// Returns 0.0 if the grid is empty.
    pub fn sample_height_at_dir(&self, dir: Vec3, grid: &SurfaceGrid) -> f32 {
        if grid.resolution == 0 {
            return 0.0;
        }
        let (face, uv) = dir_to_face_uv(dir);
        grid.sample_height_uv(face, uv)
    }
}

pub fn cube_face_dir(face: u8, uv: Vec2) -> Vec3 {
    let sx = uv.x * 2.0 - 1.0;
    let sy = uv.y * 2.0 - 1.0;
    let dir = match face {
        0 => Vec3::new(1.0, -sy, -sx),
        1 => Vec3::new(-1.0, -sy, sx),
        2 => Vec3::new(sx, 1.0, sy),
        3 => Vec3::new(sx, -1.0, -sy),
        4 => Vec3::new(sx, -sy, 1.0),
        5 => Vec3::new(-sx, -sy, -1.0),
        _ => Vec3::new(sx, -sy, 1.0),
    };
    dir.normalize_or_zero()
}

pub fn dir_to_face_uv(dir: Vec3) -> (u8, Vec2) {
    if dir.length_squared() <= f32::EPSILON {
        return (4, Vec2::splat(0.5));
    }

    let abs = dir.abs();
    let (face, major, uc, vc) = if abs.x >= abs.y && abs.x >= abs.z {
        if dir.x >= 0.0 {
            (0, abs.x, -dir.z, dir.y)
        } else {
            (1, abs.x, dir.z, dir.y)
        }
    } else if abs.y >= abs.x && abs.y >= abs.z {
        if dir.y >= 0.0 {
            (2, abs.y, dir.x, dir.z)
        } else {
            (3, abs.y, dir.x, -dir.z)
        }
    } else if dir.z >= 0.0 {
        (4, abs.z, dir.x, -dir.y)
    } else {
        (5, abs.z, -dir.x, -dir.y)
    };

    if major <= f32::EPSILON {
        return (4, Vec2::splat(0.5));
    }

    let u = 0.5 * (uc / major + 1.0);
    let v = 0.5 * (vc / major + 1.0);
    (face, Vec2::new(u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
}
