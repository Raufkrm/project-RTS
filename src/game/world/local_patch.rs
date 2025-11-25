use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::PrimitiveTopology;
use bevy_mesh::Indices;

use crate::core::galaxy_camera::{CameraMode, GalaxyCamera, CAMERA_SURFACE_CLEARANCE};
use crate::core::surface_model::{dir_to_face_uv, PlanetSurfaceModel, SurfaceCoord};
use crate::game::world::surface_grid::SurfaceGrid;

#[derive(Resource)]
pub struct LocalSurfacePatch {
    pub entity: Option<Entity>,
    pub center_coord: SurfaceCoord,
    pub angular_radius: f32, // radians
    pub resolution: u32,     // vertices per side
}

impl Default for LocalSurfacePatch {
    fn default() -> Self {
        Self {
            entity: None,
            center_coord: SurfaceCoord {
                face: 0,
                uv: Vec2::new(0.5, 0.5),
                height: 0.0,
            },
            angular_radius: 0.02,
            resolution: 128,
        }
    }
}

const PATCH_ALT_MAX: f32 = 80_000.0;
const PATCH_RECENTER_ANGLE: f32 = 0.01;

pub fn update_local_surface_patch_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut patch: ResMut<LocalSurfacePatch>,
    surface_model: Res<PlanetSurfaceModel>,
    surface_grid: Res<SurfaceGrid>,
    cameras: Query<(&Transform, &GalaxyCamera)>,
) {
    if surface_grid.resolution == 0 {
        return;
    }

    let Some((cam_transform, cam_state)) = cameras.iter().next() else {
        return;
    };

    let pos = cam_transform.translation;
    let radius = pos.length();
    if radius <= 0.0 {
        return;
    }
    let dir = pos / radius;
    let alt = radius - surface_model.radius;

    if alt > PATCH_ALT_MAX && cam_state.mode != CameraMode::FirstPerson {
        if let Some(entity) = patch.entity.take() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let (face, uv) = dir_to_face_uv(dir);
    let height = surface_grid.sample_height_uv(face, uv);
    let current_coord = SurfaceCoord { face, uv, height };

    let recenter = {
        let old_world = surface_model.surface_to_world(patch.center_coord);
        let old_dir = old_world.normalize_or_zero();
        let dot = old_dir.dot(dir).clamp(-1.0, 1.0);
        let angle = dot.acos();
        angle > PATCH_RECENTER_ANGLE
            || patch.entity.is_none()
            || (alt < 5_000.0 && cam_state.mode == CameraMode::FirstPerson)
    };

    if !recenter {
        return;
    }

    if let Some(entity) = patch.entity.take() {
        commands.entity(entity).despawn();
    }

    patch.center_coord = current_coord;

    let res = patch.resolution.max(2);
    let angular_radius = patch.angular_radius;

    let mut positions: Vec<[f32; 3]> = Vec::with_capacity((res * res) as usize);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity((res * res) as usize);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity((res * res) as usize);
    let mut indices: Vec<u32> = Vec::with_capacity(((res - 1) * (res - 1) * 6) as usize);

    let center_world = surface_model.surface_to_world(current_coord);
    let center_dir = center_world.normalize_or_zero();
    let up = center_dir;
    let right = if up.cross(Vec3::Y).length_squared() > 1e-4 {
        up.cross(Vec3::Y).normalize()
    } else {
        up.cross(Vec3::X).normalize()
    };
    let forward = right.cross(up).normalize();

    for iy in 0..res {
        let vy = (iy as f32 / (res - 1) as f32 - 0.5) * 2.0;
        let angle_y = vy * angular_radius;
        for ix in 0..res {
            let vx = (ix as f32 / (res - 1) as f32 - 0.5) * 2.0;
            let angle_x = vx * angular_radius;

            let offset_dir =
                (center_dir + right * angle_x + forward * angle_y).normalize_or_zero();

            let offset_height = surface_model.sample_height_at_dir(offset_dir, &surface_grid);

            let radius_here =
                surface_model.radius + offset_height + CAMERA_SURFACE_CLEARANCE * 0.5;

            let world_pos = offset_dir * radius_here;
            positions.push(world_pos.to_array());
            normals.push(offset_dir.to_array());

            let (_, uv_local) = dir_to_face_uv(offset_dir);
            uvs.push(uv_local.to_array());
        }
    }

    for iy in 0..(res - 1) {
        for ix in 0..(res - 1) {
            let i0 = iy * res + ix;
            let i1 = i0 + 1;
            let i2 = i0 + res;
            let i3 = i2 + 1;

            indices.push(i0);
            indices.push(i2);
            indices.push(i1);

            indices.push(i1);
            indices.push(i2);
            indices.push(i3);
        }
    }

    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));

    let mesh_handle = meshes.add(mesh);

    let material_handle = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.8),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..Default::default()
    });

    let entity = commands
        .spawn((
            Mesh3d(mesh_handle),
            MeshMaterial3d(material_handle),
            Transform::IDENTITY,
            GlobalTransform::default(),
            Visibility::Visible,
            InheritedVisibility::default(),
        ))
        .id();

    patch.entity = Some(entity);
}
