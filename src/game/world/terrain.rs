use crate::game::InGameRoot;
use bevy::math::primitives::Cuboid;
use bevy::prelude::*; // <-- add this

#[derive(Resource, Clone)]
pub struct MapSettings {
    pub width: u32,
    pub height: u32,
    pub tile_size: f32,
    pub seed: u64,
    pub water_level: f32,
}
impl Default for MapSettings {
    fn default() -> Self {
        Self {
            width: 64,
            height: 64,
            tile_size: 0.5,
            seed: 1337,
            water_level: 0.35,
        }
    }
}

#[derive(Component)]
pub struct MapRoot;

#[inline]
fn hash_u32(seed: u64, x: i32, y: i32) -> u32 {
    let mut v = seed
        ^ ((x as u64).wrapping_mul(0x9E37_79B1_85EB_CA87))
        ^ ((y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F));
    v ^= v >> 33;
    v = v.wrapping_mul(0xff51_afd7_ed55_8ccd);
    v ^= v >> 33;
    v = v.wrapping_mul(0xc4ceb9fe1a85ec53);
    v ^= v >> 33;
    (v & 0xFFFF_FFFF) as u32
}

#[inline]
fn h01(seed: u64, x: i32, y: i32) -> f32 {
    (hash_u32(seed, x, y) as f32) / (u32::MAX as f32) // 0..1
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// Value noise with bilinear interpolation (smooth, has spatial correlation).
fn value_noise(seed: u64, x: f32, y: f32) -> f32 {
    let x0 = x.floor() as i32;
    let y0 = y.floor() as i32;
    let x1 = x0 + 1;
    let y1 = y0 + 1;

    let tx = smoothstep(x - x.floor());
    let ty = smoothstep(y - y.floor());

    let v00 = h01(seed, x0, y0);
    let v10 = h01(seed, x1, y0);
    let v01 = h01(seed, x0, y1);
    let v11 = h01(seed, x1, y1);

    let a = lerp(v00, v10, tx);
    let b = lerp(v01, v11, tx);
    lerp(a, b, ty)
}

/// Fractal Brownian Motion (sum of octaves)
fn fbm(seed: u64, x: f32, y: f32) -> f32 {
    let mut amp = 1.0;
    let mut freq = 0.05; // base frequency (lower = bigger blobs)
    let mut sum = 0.0;
    let mut norm = 0.0;

    for _ in 0..4 {
        // 4 octaves is fine for now
        sum += value_noise(seed, x * freq, y * freq) * amp;
        norm += amp;
        amp *= 0.5; // gain
        freq *= 2.0; // lacunarity
    }
    (sum / norm).clamp(0.0, 1.0)
}

pub fn spawn_random_map(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    settings: &MapSettings,
) {
    let parent = commands
        .spawn((
            MapRoot,
            InGameRoot,           // <-- tag here, no generic Copy
            Transform::default(), // <-- SpatialBundle -> Transform
            Name::new("MapRoot"),
        ))
        .id();

    let w = settings.width as i32;
    let h = settings.height as i32;
    let hw = w / 2;
    let hh = h / 2;
    let ts = settings.tile_size;

    let unit_h = 0.05_f32;
    let base_mesh = meshes.add(Cuboid::new(ts, unit_h, ts));

    commands.entity(parent).with_children(|tiles| {
        for gy in 0..h {
            for gx in 0..w {
                let x = gx - hw;
                let y = gy - hh;

                // sample smooth noise at world coords
                let n = fbm(settings.seed, x as f32, y as f32);
                let (color, height) = if n < settings.water_level {
                    (Color::srgba(0.15, 0.35, 0.85, 1.0), 0.0)
                } else {
                    (Color::srgba(0.20, 0.60, 0.25, 1.0), 0.02)
                };

                let mat = materials.add(color);

                tiles.spawn((
                    Mesh3d(base_mesh.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_xyz(x as f32 * ts, height, y as f32 * ts),
                ));
            }
        }
    });
}
