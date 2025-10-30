use bevy::{
    asset::RenderAssetUsages,
    math::Vec3A,
    pbr::{MeshMaterial3d, StandardMaterial},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use bevy_mesh::{Indices, Mesh};

use super::{
    lod::PatchKey,
    render::{PatchRegistry, SurfacePatch},
    stream::PatchRequestQueue,
};
use crate::game::world::planet::{PlanetEntity, PlanetParams};

const PATCH_VERTS_PER_SIDE: usize = 12;
const PATCH_ALTITUDE_OFFSET: f32 = 150.0;
const PATCHES_PER_FRAME: u32 = 2;

/// Consume queued patch requests and spawn simple placeholder markers.
pub fn process_patch_queue(
    mut commands: Commands,
    mut queue: ResMut<PatchRequestQueue>,
    current_planet: Res<PlanetEntity>,
    params: Res<PlanetParams>,
    mut registry: ResMut<PatchRegistry>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let planet_entity = match current_planet.0 {
        Some(entity) => entity,
        None => return,
    };

    let radius = params.radius.max(1.0);

    // Limit work per frame so we don't stall when many patches enqueue.
    let mut spawned_this_frame = 0u32;
    while let Some(key) = queue.pop() {
        if registry.keys.contains(&key) {
            continue;
        }

        if let Some((position, marker_scale)) = patch_marker_transform(key, radius) {
            let mesh = build_patch_mesh(key, radius, marker_scale);
            let mesh_handle = meshes.add(mesh);
            let material_handle = materials.add(StandardMaterial {
                base_color: face_color(key.face),
                ..default()
            });

            commands.entity(planet_entity).with_children(|parent| {
                parent.spawn((
                    SurfacePatch { key },
                    Mesh3d(mesh_handle),
                    MeshMaterial3d(material_handle),
                    Transform::from_translation(position),
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

fn patch_marker_transform(key: PatchKey, radius: f32) -> Option<(Vec3, f32)> {
    let dir = cube_face_direction(key)?;
    let marker_scale = radius * 0.08 / (1u32 << key.level) as f32;
    let position = dir * (radius + PATCH_ALTITUDE_OFFSET);
    Some((position, marker_scale))
}

fn cube_face_direction(key: PatchKey) -> Option<Vec3> {
    let subdiv = 1u32 << key.level;
    if subdiv == 0 {
        return None;
    }
    let inv = 1.0 / subdiv as f32;
    let u = ((key.ix as f32 + 0.5) * inv) * 2.0 - 1.0;
    let v = ((key.iy as f32 + 0.5) * inv) * 2.0 - 1.0;

    let dir = match key.face {
        0 => Vec3::new(1.0, v, -u),
        1 => Vec3::new(-1.0, v, u),
        2 => Vec3::new(u, 1.0, -v),
        3 => Vec3::new(u, -1.0, v),
        4 => Vec3::new(u, v, 1.0),
        5 => Vec3::new(-u, v, -1.0),
        _ => return None,
    };
    Some(dir.normalize_or_zero())
}

fn face_color(face: u8) -> Color {
    match face % 6 {
        0 => Color::srgb(0.85, 0.4, 0.4),
        1 => Color::srgb(0.4, 0.75, 0.4),
        2 => Color::srgb(0.4, 0.4, 0.85),
        3 => Color::srgb(0.9, 0.7, 0.3),
        4 => Color::srgb(0.7, 0.4, 0.8),
        _ => Color::srgb(0.4, 0.8, 0.8),
    }
}

fn build_patch_mesh(key: PatchKey, radius: f32, _marker_scale: f32) -> Mesh {
    let (u0, v0, u1, v1) = patch_uv_bounds(key);
    let verts = PATCH_VERTS_PER_SIDE.max(2);

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(verts * verts);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(verts * verts);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(verts * verts);

    for j in 0..verts {
        let v = lerp(v0, v1, j as f32 / (verts - 1) as f32);
        for i in 0..verts {
            let u = lerp(u0, u1, i as f32 / (verts - 1) as f32);
            let dir = cube_uv_to_dir(key.face, u, v);
            let pos = dir * (radius + PATCH_ALTITUDE_OFFSET);
            positions.push(pos.to_array());
            normals.push(dir.to_array());
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

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn patch_uv_bounds(key: PatchKey) -> (f32, f32, f32, f32) {
    let subdiv = 1u32 << key.level;
    let inv = 2.0 / subdiv as f32;
    let u0 = -1.0 + key.ix as f32 * inv;
    let v0 = -1.0 + key.iy as f32 * inv;
    let u1 = u0 + inv;
    let v1 = v0 + inv;
    (u0, v0, u1, v1)
}

fn cube_uv_to_dir(face: u8, u: f32, v: f32) -> Vec3 {
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

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
