use bevy::prelude::*;
use std::collections::HashSet;

use crate::core::camera::EditorCamera;
use super::patch::{Patch, PatchId, PatchGrid};
use super::sampling::{WorldSampler, FlatSamplerRes};

// Public in 0.17:
use bevy::render::render_resource::PrimitiveTopology;
use bevy::asset::RenderAssetUsages;

#[derive(Resource, Default)]
pub struct WantedPatches(pub HashSet<PatchId>);

pub fn compute_wanted_patches(
    grid: Res<PatchGrid>,
    cams: Query<&EditorCamera>,
    mut wanted: ResMut<WantedPatches>,
) {
    // 0.17: use `single()`
    let Ok(cam) = cams.single() else { return; };
    wanted.0.clear();

    let s = grid.patch_size_m;
    let gx = (cam.focus.x / s).floor() as i32;
    let gy = (cam.focus.z / s).floor() as i32;

    for dy in -grid.visible_radius..=grid.visible_radius {
        for dx in -grid.visible_radius..=grid.visible_radius {
            wanted.0.insert(PatchId { gx: gx + dx, gy: gy + dy });
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

    // spawn missing
    for id in wanted.0.iter() {
        if have.contains(id) { continue; }
        spawn_one_patch(&mut commands, &mut meshes, &mut materials, *id, &grid, &sampler.0);
    }

    // despawn far
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
    // We build a NON-INDEXED mesh (triangles with duplicated vertices)
    // to avoid private Indices type in Bevy 0.17.
    let n = grid.verts_per_side.max(2);
    let s = grid.patch_size_m;
    let half = s * 0.5;

    let ox = (id.gx as f32) * s;
    let oz = (id.gy as f32) * s;
    let step = s / (n - 1) as f32;

    // Precompute heights on grid points
    let mut heights = vec![0.0f32; (n * n) as usize];
    let idx = |i: u32, j: u32| -> usize { (j * n + i) as usize };
    for j in 0..n {
        for i in 0..n {
            let x_local = -half + i as f32 * step;
            let z_local = -half + j as f32 * step;
            let xw = ox + x_local;
            let zw = oz + z_local;
            let s = sampler.sample(xw, zw);
            heights[idx(i, j)] = s.height;
        }
    }

    // Now build triangles
    // For each cell (i,j) we create two triangles: (a,c,b) and (b,c,d)
    // where a=(i,j), b=(i+1,j), c=(i,j+1), d=(i+1,j+1)
    let quads = (n - 1) * (n - 1);
    let tri_count = quads * 2;
    let vert_count = tri_count * 3;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vert_count as usize);
    let mut normals:   Vec<[f32; 3]> = Vec::with_capacity(vert_count as usize);
    let mut uvs:       Vec<[f32; 2]> = Vec::with_capacity(vert_count as usize);

    let uv = |i: u32, j: u32| -> [f32; 2] {
        [i as f32 / (n - 1) as f32, j as f32 / (n - 1) as f32]
    };
    let pos = |i: u32, j: u32| -> [f32; 3] {
        let x_local = -half + i as f32 * step;
        let z_local = -half + j as f32 * step;
        let y = heights[idx(i, j)];
        [x_local, y, z_local]
    };

    for j in 0..(n - 1) {
        for i in 0..(n - 1) {
            let a = pos(i, j);
            let b = pos(i + 1, j);
            let c = pos(i, j + 1);
            let d = pos(i + 1, j + 1);
            let ua = uv(i, j);
            let ub = uv(i + 1, j);
            let uc = uv(i, j + 1);
            let ud = uv(i + 1, j + 1);

            // Triangle 1: a, c, b
            positions.push(a); uvs.push(ua);
            positions.push(c); uvs.push(uc);
            positions.push(b); uvs.push(ub);

            // Triangle 2: b, c, d
            positions.push(b); uvs.push(ub);
            positions.push(c); uvs.push(uc);
            positions.push(d); uvs.push(ud);

            // Simple up normals for all 6 verts (compute later if needed)
            for _ in 0..6 {
                normals.push([0.0, 1.0, 0.0]);
            }
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    // No indices: non-indexed draw

    let mesh_h = meshes.add(mesh);
    let mat_h = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.42, 0.24),
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
