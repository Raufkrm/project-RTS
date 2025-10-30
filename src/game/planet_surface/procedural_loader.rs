use bevy::{
    asset::RenderAssetUsages,
    pbr::{MeshMaterial3d, StandardMaterial},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use bevy_mesh::{Indices, Mesh};

use super::{
    lod::{self, PatchKey},
    manager::{PlanetContext, PlanetContextLayer},
    render::{PatchRegistry, SurfacePatch},
    stream::PatchRequestQueue,
};
use crate::game::world::{
    planet::{PlanetEntity, PlanetParams},
    sampling::{FlatSamplerRes, WorldSampler},
};

const PATCH_VERTS_PER_SIDE: usize = 12;
const PATCHES_PER_FRAME: u32 = 16;

/// Consume queued patch requests and spawn simple placeholder markers.
pub fn process_patch_queue(
    mut commands: Commands,
    mut queue: ResMut<PatchRequestQueue>,
    current_planet: Res<PlanetEntity>,
    params: Res<PlanetParams>,
    sampler: Res<FlatSamplerRes>,
    mut registry: ResMut<PatchRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    context: Res<PlanetContext>,
) {
    let planet_entity = match current_planet.0 {
        Some(entity) => entity,
        None => return,
    };

    if !matches!(
        context.layer,
        PlanetContextLayer::Approach | PlanetContextLayer::Surface
    ) {
        return;
    }

    let radius = params.radius.max(1.0);

    // Limit work per frame so we don't stall when many patches enqueue.
    let mut spawned_this_frame = 0u32;
    while let Some(key) = queue.pop() {
        if !context.desired.contains(&key) {
            continue;
        }
        if registry.keys.contains(&key) {
            continue;
        }

        if let Some(mesh) = build_patch_mesh(key, radius, &*sampler, params.height_amp) {
            let mesh_handle = meshes.add(mesh);
            let tint = patch_debug_color(key);
            let material_handle = materials.add(StandardMaterial {
                base_color: tint,
                unlit: true,
                perceptual_roughness: 1.0,
                ..default()
            });

            commands.entity(planet_entity).with_children(|parent| {
                parent.spawn((
                    SurfacePatch { key },
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                    Transform::IDENTITY,
                    Visibility::Visible,
                    Name::new(format!(
                        "SurfacePatch face{}-lvl{}-({},{})",
                        key.face, key.level, key.ix, key.iy
                    )),
                ));
            });

            registry.keys.insert(key);
            spawned_this_frame += 1;
            if spawned_this_frame >= PATCHES_PER_FRAME {
                break;
            }
        }
    }
}

fn patch_debug_color(key: PatchKey) -> Color {
    let base = match key.face % 6 {
        0 => Vec3::new(0.85, 0.4, 0.4),
        1 => Vec3::new(0.4, 0.75, 0.4),
        2 => Vec3::new(0.4, 0.4, 0.85),
        3 => Vec3::new(0.9, 0.7, 0.3),
        4 => Vec3::new(0.7, 0.4, 0.8),
        _ => Vec3::new(0.4, 0.8, 0.8),
    };
    let mix = (key.level as f32).min(5.0) * 0.08;
    let tinted = base * (1.0 - mix) + Vec3::splat(mix);
    Color::srgb(
        tinted.x.clamp(0.0, 1.0),
        tinted.y.clamp(0.0, 1.0),
        tinted.z.clamp(0.0, 1.0),
    )
}

fn build_patch_mesh(
    key: PatchKey,
    radius: f32,
    sampler: &FlatSamplerRes,
    height_scale: f32,
) -> Option<Mesh> {
    let (u0, v0, u1, v1) = lod::patch_uv_bounds(key);
    let verts = PATCH_VERTS_PER_SIDE.max(2);

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(verts * verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(verts * verts);

    for j in 0..verts {
        let v = lerp(v0, v1, j as f32 / (verts - 1) as f32);
        for i in 0..verts {
            let u = lerp(u0, u1, i as f32 / (verts - 1) as f32);
            let dir = lod::cube_uv_to_dir(key.face, u, v);
            let world_pos = dir * radius;
            let sample = sampler.0.sample(world_pos.x, world_pos.z);
            let height = sample.height * height_scale;
            positions.push((dir * (radius + height)).to_array());
            uvs.push([i as f32 / (verts - 1) as f32, j as f32 / (verts - 1) as f32]);
        }
    }

    let mut indices: Vec<u32> = Vec::with_capacity((verts - 1) * (verts - 1) * 6);
    for j in 0..(verts - 1) {
        for i in 0..(verts - 1) {
            let a = (j * verts + i) as u32;
            let b = a + 1;
            let c = a + verts as u32;
            let d = c + 1;
            indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }

    let mut normal_accum: Vec<Vec3> = vec![Vec3::ZERO; positions.len()];
    for tri in indices.chunks_exact(3) {
        let idx0 = tri[0] as usize;
        let idx1 = tri[1] as usize;
        let idx2 = tri[2] as usize;

        let p0 = Vec3::from_array(positions[idx0]);
        let p1 = Vec3::from_array(positions[idx1]);
        let p2 = Vec3::from_array(positions[idx2]);

        let normal = (p1 - p0).cross(p2 - p0);
        if normal.length_squared() == 0.0 {
            continue;
        }

        normal_accum[idx0] += normal;
        normal_accum[idx1] += normal;
        normal_accum[idx2] += normal;
    }

    let normals: Vec<[f32; 3]> = normal_accum
        .into_iter()
        .enumerate()
        .map(|(i, n)| {
            let fallback = Vec3::from_array(positions[i]).normalize_or_zero();
            n.try_normalize().unwrap_or(fallback).to_array()
        })
        .collect();

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn prune_surface_patches(
    mut commands: Commands,
    context: Res<PlanetContext>,
    patches: Query<(Entity, &SurfacePatch)>,
) {
    match context.layer {
        PlanetContextLayer::Approach | PlanetContextLayer::Surface => {
            for (entity, patch) in patches.iter() {
                if !context.desired.contains(&patch.key) {
                    commands.entity(entity).despawn();
                }
            }
        }
        PlanetContextLayer::Orbit => {
            for (entity, _) in patches.iter() {
                commands.entity(entity).despawn();
            }
        }
    }
}
