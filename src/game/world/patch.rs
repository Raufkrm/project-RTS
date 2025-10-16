use bevy::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PatchId { pub gx: i32, pub gy: i32 }

#[derive(Component)]
pub struct Patch { pub id: PatchId }

#[derive(Resource)]
pub struct PatchGrid {
    pub patch_size_m: f32,   // meters per side
    pub visible_radius: i32, // patches around camera focus
    pub verts_per_side: u32, // tessellation
}

impl Default for PatchGrid {
    fn default() -> Self {
        Self {
            patch_size_m: 256.0,
            visible_radius: 4,  // ← was 3
            verts_per_side: 64,
        }
    }
}
