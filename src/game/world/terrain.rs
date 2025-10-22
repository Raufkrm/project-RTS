//! Smooth heightmapped terrain + water plane for Bevy 0.17,
//! with reroll (press R) + randomized/locally-varying water level.

use crate::core::camera::EditorCamera;
use crate::game::world::patch::{Patch, PatchGrid, PatchId};
use crate::game::world::sampling::{FlatSamplerRes, Sample, WorldSampler};
use crate::game::InGameRoot;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::prelude::*;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use std::time::{SystemTime, UNIX_EPOCH};

/// Tunables for the procedural map.
#[derive(Resource, Clone)]
pub struct MapSettings {
    pub width: u32,
    pub height: u32,
    pub tile_size: f32,
    pub seed: u64,

    /// World Y = (noise - effective_water_level) * height_amplitude
    pub water_level: f32,

    /// When > 0, the *effective* water level varies in space:
    /// effective_water = water_level + water_var_amp * (water_noise - 0.5)
    pub water_var_amp: f32, // try 0.04..0.12
    pub water_var_freq: f32, // try 0.005..0.02 (lower = larger basins)

    /// Height scale in world units.
    pub height_amplitude: f32,

    /// Base noise frequency for terrain (lower => larger landmasses)
    pub base_freq: f32,

    /// Range used when randomizing the global water_level each reroll
    pub water_min: f32,
    pub water_max: f32,
}

impl Default for MapSettings {
    fn default() -> Self {
        Self {
            width: 64,
            height: 64,
            tile_size: 0.5,
            seed: 1337,

            water_level: 0.35,
            water_var_amp: 0.08,
            water_var_freq: 0.01,

            height_amplitude: 1.0,
            base_freq: 0.05,

            water_min: 0.25,
            water_max: 0.5,
        }
    }
}

#[derive(Component)]
pub struct MapRoot;
#[derive(Resource, Default)]
struct WantedPatches(pub std::collections::HashSet<PatchId>);

// ---------- noise ----------

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
    (hash_u32(seed, x, y) as f32) / (u32::MAX as f32)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
#[inline]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

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

fn fbm(seed: u64, x: f32, y: f32, base_freq: f32, octaves: u32, gain: f32, lacunarity: f32) -> f32 {
    let mut amp = 1.0;
    let mut freq = base_freq.max(0.000_01);
    let mut sum = 0.0;
    let mut norm = 0.0;
    for _ in 0..octaves {
        sum += value_noise(seed, x * freq, y * freq) * amp;
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    (sum / norm).clamp(0.0, 1.0)
}

/// Spatially varying water level around the base water_level.
#[inline]
fn effective_water_level(settings: &MapSettings, seed: u64, wx: f32, wz: f32) -> f32 {
    if settings.water_var_amp <= 0.0 {
        return settings.water_level;
    }
    let mask = value_noise(
        seed.wrapping_add(0xBEEF),
        wx * settings.water_var_freq,
        wz * settings.water_var_freq,
    );
    settings.water_level + settings.water_var_amp * (mask - 0.5)
}

/// Convert noise to world height (relative to *effective* local water level).
#[inline]
fn height_from_noise(settings: &MapSettings, n01: f32, eff_water: f32) -> f32 {
    (n01 - eff_water) * settings.height_amplitude
}

/// Quick-n-dirty new seed.
fn next_seed(old: u64) -> u64 {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    old ^ t ^ 0x9E37_79B9_7F4A_7C15
}

/// Press R to despawn the old map and build a new one with a new seed + water level.
pub fn reroll_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut settings: ResMut<MapSettings>,
    roots: Query<Entity, With<MapRoot>>,
    children_q: Query<&Children>,
) {
    if !keys.just_pressed(KeyCode::KeyR) {
        return;
    }

    // Despawn previous map(s). In 0.17, despawn() removes children too.
    for e in &roots {
        despawn_recursive(&mut commands, e, &children_q); // <-- use helper
    }

    // Randomize seed and global water level in the configured range
    settings.seed = next_seed(settings.seed);
    let rnd01 = (hash_u32(settings.seed, 101, 303) as f32) / (u32::MAX as f32);
    settings.water_level = settings.water_min + rnd01 * (settings.water_max - settings.water_min);

    // Rebuild
    spawn_random_map(&mut commands, &mut meshes, &mut materials, &settings);
}

/// Build a smooth terrain mesh (triangle grid) and a transparent water quad at Y=0.
pub fn spawn_random_map(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    settings: &MapSettings,
) {
    let parent = commands
        .spawn((
            MapRoot,
            InGameRoot,
            Transform::default(),
            Name::new("MapRoot"),
        ))
        .id();

    let w = settings.width as usize;
    let h = settings.height as usize;
    let nx = w + 1;
    let nz = h + 1;
    let ts = settings.tile_size;

    let total_w = w as f32 * ts;
    let total_h = h as f32 * ts;
    let x0 = -0.5 * total_w;
    let z0 = -0.5 * total_h;

    // heights (for normals)
    let mut heights = vec![0.0f32; nx * nz];
    for z in 0..nz {
        for x in 0..nx {
            let wx = x0 + (x as f32) * ts;
            let wz = z0 + (z as f32) * ts;

            let n = fbm(settings.seed, wx, wz, settings.base_freq, 5, 0.5, 2.0);
            let eff_water = effective_water_level(settings, settings.seed, wx, wz);
            heights[z * nx + x] = height_from_noise(settings, n, eff_water);
        }
    }

    let mut positions = Vec::with_capacity(nx * nz);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(nx * nz);
    let mut uvs = Vec::with_capacity(nx * nz);

    let h_at = |x: isize, z: isize| -> f32 {
        let xi = x.clamp(0, (nx - 1) as isize) as usize;
        let zi = z.clamp(0, (nz - 1) as isize) as usize;
        heights[zi * nx + xi]
    };

    for z in 0..nz {
        for x in 0..nx {
            let wx = x0 + (x as f32) * ts;
            let wz = z0 + (z as f32) * ts;
            let y = heights[z * nx + x];

            let hl = h_at(x as isize - 1, z as isize);
            let hr = h_at(x as isize + 1, z as isize);
            let hd = h_at(x as isize, z as isize - 1);
            let hu = h_at(x as isize, z as isize + 1);
            let sx = (hr - hl) / (2.0 * ts);
            let sz = (hu - hd) / (2.0 * ts);
            let n = Vec3::new(-sx, 1.0, -sz).normalize();

            positions.push([wx, y, wz]);
            normals.push(n.into());
            uvs.push([x as f32 / w.max(1) as f32, z as f32 / h.max(1) as f32]);
        }
    }

    // indices
    let mut indices_vec: Vec<u32> = Vec::with_capacity(w * h * 6);
    for z in 0..h {
        for x in 0..w {
            let i0 = (z * nx + x) as u32;
            let i1 = i0 + 1;
            let i2 = (z * nx + x + nx) as u32;
            let i3 = i2 + 1;
            indices_vec.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
        }
    }

    // terrain mesh
    let mut terrain_mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    terrain_mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    terrain_mesh.insert_indices(Indices::U32(indices_vec));

    let terrain_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.20, 0.60, 0.25, 1.0),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });

    let terrain_entity = commands
        .spawn((
            Mesh3d(meshes.add(terrain_mesh)),
            MeshMaterial3d(terrain_mat),
            Transform::default(),
            Name::new("Terrain"),
        ))
        .id();

    // water (single quad at y=0)
    let mut water = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    let hw = 0.5 * total_w;
    let hh = 0.5 * total_h;
    water.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-hw, 0.0, -hh],
            [hw, 0.0, -hh],
            [hw, 0.0, hh],
            [-hw, 0.0, hh],
        ],
    );
    water.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4]);
    water.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
    );
    // reversed winding (CCW from above)
    water.insert_indices(Indices::U32(vec![0, 2, 1, 0, 3, 2]));

    let water_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.15, 0.35, 0.85, 0.7),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.05,
        metallic: 0.0,
        ..default()
    });

    let water_entity = commands
        .spawn((
            Mesh3d(meshes.add(water)),
            MeshMaterial3d(water_mat),
            Transform::default(),
            Name::new("Water"),
        ))
        .id();

    commands
        .entity(parent)
        .add_children(&[terrain_entity, water_entity]);
}
fn despawn_recursive(commands: &mut Commands, entity: Entity, children_q: &Query<&Children>) {
    if let Ok(children) = children_q.get(entity) {
        for child in children.iter() {
            despawn_recursive(commands, child, children_q); // <- just child
        }
    }
    commands.entity(entity).despawn();
}
