use bevy::prelude::*;

#[inline]
pub fn idx(resolution: u32, x: u32, y: u32) -> usize {
    (y * resolution + x) as usize
}

#[derive(Resource)]
pub struct SurfaceGrid {
    /// Resolution per face (number of cells along one side).
    pub resolution: u32, // e.g. 1024
    /// Per-face height samples in meters above base radius.
    /// Each Vec has resolution * resolution elements, row-major [y][x].
    pub heights: [Vec<f32>; 6],
    /// Per-face biome ids (same ids you already use for shaders / biome classifier).
    pub biomes: [Vec<u8>; 6],
}

impl Default for SurfaceGrid {
    fn default() -> Self {
        let resolution = 0;
        let empty_f32: Vec<f32> = Vec::new();
        let empty_u8: Vec<u8> = Vec::new();
        SurfaceGrid {
            resolution,
            heights: [
                empty_f32.clone(),
                empty_f32.clone(),
                empty_f32.clone(),
                empty_f32.clone(),
                empty_f32.clone(),
                empty_f32.clone(),
            ],
            biomes: [
                empty_u8.clone(),
                empty_u8.clone(),
                empty_u8.clone(),
                empty_u8.clone(),
                empty_u8.clone(),
                empty_u8.clone(),
            ],
        }
    }
}

impl SurfaceGrid {
    #[inline]
    fn index(&self, face: u8, x: u32, y: u32) -> usize {
        let _ = face; // face is stored in separate vectors
        idx(self.resolution, x, y)
    }

    /// Get raw height by integer indices (no bounds check).
    #[inline]
    pub fn height_at(&self, face: u8, x: u32, y: u32) -> f32 {
        if self.resolution == 0 {
            return 0.0;
        }
        let f = face as usize;
        let i = idx(self.resolution, x, y);
        self.heights[f][i]
    }

    /// Get raw biome id by integer indices (no bounds check).
    #[inline]
    pub fn biome_at(&self, face: u8, x: u32, y: u32) -> u8 {
        let f = face as usize;
        let i = idx(self.resolution, x, y);
        self.biomes[f][i]
    }

    /// Sample height using uv in [0,1]x[0,1] (clamped), nearest neighbour.
    pub fn sample_height_uv(&self, face: u8, uv: Vec2) -> f32 {
        if self.resolution == 0 {
            return 0.0;
        }

        let res = self.resolution as f32;
        let fx = (uv.x.clamp(0.0, 0.9999)) * res;
        let fy = (uv.y.clamp(0.0, 0.9999)) * res;

        let x0 = fx.floor() as u32;
        let y0 = fy.floor() as u32;
        let x1 = (x0 + 1).min(self.resolution - 1);
        let y1 = (y0 + 1).min(self.resolution - 1);

        let tx = fx - x0 as f32;
        let ty = fy - y0 as f32;

        let h00 = self.height_at(face, x0, y0);
        let h10 = self.height_at(face, x1, y0);
        let h01 = self.height_at(face, x0, y1);
        let h11 = self.height_at(face, x1, y1);

        let h0 = h00.lerp(h10, tx);
        let h1 = h01.lerp(h11, tx);
        h0.lerp(h1, ty)
    }

    /// Sample biome id using uv in [0,1]x[0,1] (clamped), nearest neighbour.
    pub fn sample_biome_uv(&self, face: u8, uv: Vec2) -> u8 {
        if self.resolution == 0 {
            return 0;
        }
        let u = uv.x.clamp(0.0, 0.999_999);
        let v = uv.y.clamp(0.0, 0.999_999);
        let x = (u * self.resolution as f32).floor() as u32;
        let y = (v * self.resolution as f32).floor() as u32;
        self.biome_at(face, x, y)
    }
}
