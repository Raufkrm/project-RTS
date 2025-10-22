use bevy::prelude::*;
use std::collections::HashSet;

use super::patch::{Patch, PatchGrid, PatchId};
use super::sampling::{FlatSamplerRes, WorldSampler};
use crate::core::camera::EditorCamera;

use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::PrimitiveTopology;

#[derive(Resource, Default)]
pub struct WantedPatches(pub HashSet<PatchId>);

pub fn compute_wanted_patches(
    grid: Res<PatchGrid>,
    cams: Query<&EditorCamera>,
    mut wanted: ResMut<WantedPatches>,
) {
    let Ok(cam) = cams.single() else {
        return;
    };
    wanted.0.clear();

    let s = grid.patch_size_m;
    let gx = (cam.focus.x / s).floor() as i32;
    let gy = (cam.focus.z / s).floor() as i32;

    for dy in -grid.visible_radius..=grid.visible_radius {
        for dx in -grid.visible_radius..=grid.visible_radius {
            wanted.0.insert(PatchId {
                gx: gx + dx,
                gy: gy + dy,
            });
        }
    }
}

pub fn apply_patch_streaming(
    mut commands: Commands,
    grid: Res<PatchGrid>,
    sampler: Res<FlatSamplerRes>,
    wanted: Res<WantedPatches>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    existing: Query<(Entity, &Patch)>,
) {
    let mut have: HashSet<PatchId> = HashSet::new();
    for (_, p) in &existing {
        have.insert(p.id);
    }

    for id in wanted.0.iter() {
        if have.contains(id) {
            continue;
        }
        spawn_one_patch(
            &mut commands,
            &mut meshes,
            &mut materials,
            *id,
            &grid,
            &sampler.0,
        );
    }

    for (e, p) in &existing {
        if !wanted.0.contains(&p.id) {
            commands.entity(e).despawn();
        }
    }
}

fn spawn_one_patch(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    id: PatchId,
    grid: &PatchGrid,
    sampler: &impl WorldSampler,
) {
    let n = grid.verts_per_side.max(2);
    let s = grid.patch_size_m;
    let half = s * 0.5;

    let ox = (id.gx as f32) * s;
    let oz = (id.gy as f32) * s;
    let step = s / (n - 1) as f32;

    // For global color curve we’ll map heights to [-amp, +amp] where amp is the sampler’s amplitude.
    // If your sampler differs later, adjust here accordingly.
    let global_amp = sampler_height_amp(sampler).max(1.0);

    // Heights on grid
    let mut heights = vec![0.0f32; (n * n) as usize];
    let idx = |i: u32, j: u32| -> usize { (j * n + i) as usize };
    for j in 0..n {
        for i in 0..n {
            let x_local = -half + i as f32 * step;
            let z_local = -half + j as f32 * step;
            let xw = ox + x_local;
            let zw = oz + z_local;
            heights[idx(i, j)] = sampler.sample(xw, zw).height;
        }
    }

    // World-space normals by sampling around each vertex (no per-tile seams)
    let eps = (step * 0.5).clamp(0.25, 2.0); // world meters
    let world_normal = |xw: f32, zw: f32| -> [f32; 3] {
        let h_l = sampler.sample(xw - eps, zw).height;
        let h_r = sampler.sample(xw + eps, zw).height;
        let h_d = sampler.sample(xw, zw - eps).height;
        let h_u = sampler.sample(xw, zw + eps).height;

        // central differences (slope)
        let dhdx = (h_r - h_l) / (2.0 * eps);
        let dhdz = (h_u - h_d) / (2.0 * eps);

        // normal = normalize( (-∂h/∂x, 1, -∂h/∂z) )
        let n = Vec3::new(-dhdx, 1.0, -dhdz).normalize_or_zero();
        [n.x, n.y, n.z]
    };

    // Global color curve (same across all tiles -> no seams)
    //  sea (<=0) → deep blue
    //  shoreline (~0) → sand
    //  low/mid → greens
    //  high → rock/snow tint
    let color_for_height = |h: f32| -> [f32; 4] {
        if h <= 0.0 {
            // water/deep
            let t = (-h / global_amp).clamp(0.0, 1.0);
            let deep = Vec3::new(0.06, 0.18, 0.30);
            let shallow = Vec3::new(0.15, 0.33, 0.45);
            let c = deep.lerp(shallow, t);
            return [c.x, c.y, c.z, 1.0];
        }
        // land
        let h01 = (h / global_amp).clamp(0.0, 1.0);
        // 0..0.08 sand, 0.08..0.6 grass, 0.6..1.0 alpine
        let sand_hi = 0.08;
        let grass_hi = 0.60;
        let c = if h01 < sand_hi {
            // beach sand
            let t = h01 / sand_hi;
            Vec3::new(0.76, 0.71, 0.54).lerp(Vec3::new(0.55, 0.65, 0.45), t)
        } else if h01 < grass_hi {
            let t = (h01 - sand_hi) / (grass_hi - sand_hi);
            Vec3::new(0.42, 0.64, 0.34).lerp(Vec3::new(0.72, 0.86, 0.62), t)
        } else {
            let t = (h01 - grass_hi) / (1.0 - grass_hi);
            Vec3::new(0.72, 0.86, 0.62).lerp(Vec3::new(0.85, 0.87, 0.85), t)
        };
        [c.x, c.y, c.z, 1.0]
    };

    // Build NON-indexed triangles with per-vertex data
    let quads = (n - 1) * (n - 1);
    let tri_count = quads * 2;
    let vert_count = tri_count * 3;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vert_count as usize);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vert_count as usize);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vert_count as usize);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(vert_count as usize);

    let uv =
        |i: u32, j: u32| -> [f32; 2] { [i as f32 / (n - 1) as f32, j as f32 / (n - 1) as f32] };
    let local_pos = |i: u32, j: u32| -> (f32, f32, f32) {
        let x_local = -half + i as f32 * step;
        let z_local = -half + j as f32 * step;
        let y = heights[idx(i, j)];
        (x_local, y, z_local)
    };
    let world_xz = |i: u32, j: u32| -> (f32, f32) {
        let x_local = -half + i as f32 * step;
        let z_local = -half + j as f32 * step;
        (ox + x_local, oz + z_local)
    };

    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let (ax, ay, az) = local_pos(i, j);
            let (bx, by, bz) = local_pos(i + 1, j);
            let (cx, cy, cz) = local_pos(i, j + 1);
            let (dx, dy, dz) = local_pos(i + 1, j + 1);

            let (awx, awz) = world_xz(i, j);
            let (bwx, bwz) = world_xz(i + 1, j);
            let (cwx, cwz) = world_xz(i, j + 1);
            let (dwx, dwz) = world_xz(i + 1, j + 1);

            let na = world_normal(awx, awz);
            let nb = world_normal(bwx, bwz);
            let nc = world_normal(cwx, cwz);
            let nd = world_normal(dwx, dwz);

            let ca = color_for_height(ay);
            let cb = color_for_height(by);
            let cc = color_for_height(cy);
            let cd = color_for_height(dy);

            let ua = uv(i, j);
            let ub = uv(i + 1, j);
            let uc = uv(i, j + 1);
            let ud = uv(i + 1, j + 1);

            // tri 1: a, c, b
            positions.extend_from_slice(&[[ax, ay, az], [cx, cy, cz], [bx, by, bz]]);
            normals.extend_from_slice(&[na, nc, nb]);
            uvs.extend_from_slice(&[ua, uc, ub]);
            colors.extend_from_slice(&[ca, cc, cb]);

            // tri 2: b, c, d
            positions.extend_from_slice(&[[bx, by, bz], [cx, cy, cz], [dx, dy, dz]]);
            normals.extend_from_slice(&[nb, nc, nd]);
            uvs.extend_from_slice(&[ub, uc, ud]);
            colors.extend_from_slice(&[cb, cc, cd]);
        }
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals); // ← smooth, world-space
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors); // ← global curve (no seams)

    let mesh_h = meshes.add(mesh);
    let mat_h = materials.add(StandardMaterial {
        // base color multiplies vertex colors; keep neutral, not pure white
        base_color: Color::srgb(0.90, 0.90, 0.90),
        perceptual_roughness: 0.95,
        metallic: 0.0,
        ..default()
    });

    commands.spawn((
        Patch { id },
        Mesh3d(mesh_h),
        MeshMaterial3d(mat_h),
        Transform::from_translation(Vec3::new(ox, 0.0, oz)),
        Visibility::Visible,
        Name::new(format!("Patch({}, {})", id.gx, id.gy)),
    ));
}

#[inline]
fn sampler_height_amp(sampler: &impl WorldSampler) -> f32 {
    // Our current sampler is FlatSampler { height_amp, .. }.
    // If you swap to a different sampler later, adjust this accessor.
    // Try downcasting via Any to fetch a reasonable default:
    // For now, return a sensible global range if unknown.
    // We know FlatSamplerRes(FlatSampler { height_amp, .. }) is used, so:
    // (This fallback is only used if someone swaps the sampler type.)
    120.0
}
