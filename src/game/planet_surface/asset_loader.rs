//! Disk-backed patch asset loader.
//!
//! The streaming pipeline prefers authored bundles stored under
//! `assets/planet_patches/`.  Each bundle is described by a small JSON file
//! keyed by the patch identifier `(face, level, ix, iy)`.  When present, the
//! descriptor is converted into a [`GeneratedPatch`] so the runtime can skip
//! the expensive climate sampling path.
//!
//! The current stub supports a lightweight `simple_sphere` variant that builds
//! a curved patch directly on the planet surface.  Additional descriptor types
//! can be added later without touching the streaming core.

use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use bevy::prelude::*;
use bevy_mesh::Mesh;
use serde::Deserialize;

use super::{
    lod::{self, PatchKey},
    stream::{GeneratedPatch, PatchPropPlaceholder},
};

/// Location of streamed patch bundles on disk (relative to the executable).
const DEFAULT_PATCH_ASSET_DIR: &str = "assets/planet_patches";

/// Tracks bundle availability and configuration.
#[derive(Resource, Debug)]
pub struct PatchAssetState {
    base_path: PathBuf,
    missing: HashSet<PatchKey>,
}

impl Default for PatchAssetState {
    fn default() -> Self {
        Self {
            base_path: PathBuf::from(DEFAULT_PATCH_ASSET_DIR),
            missing: HashSet::new(),
        }
    }
}

impl PatchAssetState {
    #[inline]
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    #[inline]
    pub fn is_marked_missing(&self, key: &PatchKey) -> bool {
        self.missing.contains(key)
    }

    #[inline]
    pub fn mark_missing(&mut self, key: PatchKey) {
        self.missing.insert(key);
    }
}

/// Descriptor authored on disk.  Extendable as more bundle types appear.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PatchBundleDescriptor {
    /// Generates a curved patch using the cube-sphere layout for the given key.
    /// Resolution defaults to 18x18 vertices (matching the procedural baseline).
    SimpleSphere {
        #[serde(default = "default_resolution")]
        resolution: u32,
        #[serde(default)]
        height_offset: f32,
        #[serde(default)]
        materials: Vec<String>,
        #[serde(default)]
        props: Vec<RawPropPlaceholder>,
        #[serde(default)]
        albedo: Option<String>,
        #[serde(default)]
        normal: Option<String>,
        #[serde(default)]
        roughness: Option<String>,
    },
}

const fn default_resolution() -> u32 {
    18
}

#[derive(Debug, Deserialize)]
pub struct RawPropPlaceholder {
    pub kind: String,
    #[serde(default)]
    pub position: [f32; 3],
    #[serde(default)]
    pub rotation: [f32; 3],
    #[serde(default = "default_unit_scale")]
    pub scale: [f32; 3],
}

impl From<RawPropPlaceholder> for PatchPropPlaceholder {
    fn from(value: RawPropPlaceholder) -> Self {
        Self {
            kind: value.kind,
            position: Vec3::new(value.position[0], value.position[1], value.position[2]),
            rotation_euler: Vec3::new(value.rotation[0], value.rotation[1], value.rotation[2]),
            scale: Vec3::new(value.scale[0], value.scale[1], value.scale[2]),
        }
    }
}

const fn default_unit_scale() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}

fn normalize_asset_path(base_path: &Path, descriptor_dir: &Path, raw: &str) -> String {
    let cleaned = raw.replace('\\', "/");
    if cleaned.starts_with("assets/") {
        return cleaned.trim_start_matches("assets/").to_string();
    }
    if cleaned.starts_with("planet_patches/") {
        return cleaned;
    }

    let mut candidate = PathBuf::from(&cleaned);
    if candidate.is_relative() {
        let first_component = candidate.components().next();
        let descriptor_name = descriptor_dir.file_name().and_then(|n| n.to_str());
        match first_component {
            Some(Component::Normal(name))
                if Some(name.to_str().unwrap_or_default()) == descriptor_name =>
            {
                candidate = base_path.join(candidate);
            }
            _ => {
                candidate = descriptor_dir.join(candidate);
            }
        }
    }

    if let Ok(stripped) = candidate.strip_prefix(Path::new("assets")) {
        stripped.to_string_lossy().replace('\\', "/")
    } else {
        candidate.to_string_lossy().replace('\\', "/")
    }
}

/// Attempts to load an authored patch bundle for `key`.  `planet_radius` is
/// required so the stub can project onto the correct sphere.
pub fn load_patch_bundle_from_disk(
    base_path: PathBuf,
    key: PatchKey,
    planet_radius: f32,
) -> Option<GeneratedPatch> {
    let path = descriptor_path(&base_path, key);
    let data = match fs::read(&path) {
        Ok(data) => data,
        Err(err) => {
            if err.kind() != std::io::ErrorKind::NotFound {
                warn!(
                    "Failed to read patch descriptor {:?}: {}",
                    path.display(),
                    err
                );
            }
            return None;
        }
    };

    let descriptor: PatchBundleDescriptor = match serde_json::from_slice(&data) {
        Ok(value) => value,
        Err(err) => {
            warn!(
                "Failed to parse patch descriptor {:?}: {}",
                path.display(),
                err
            );
            return None;
        }
    };

    let descriptor_dir = path
        .parent()
        .unwrap_or_else(|| base_path.as_path())
        .to_path_buf();

    match descriptor {
        PatchBundleDescriptor::SimpleSphere {
            resolution,
            height_offset,
            mut materials,
            props,
            albedo,
            normal,
            roughness,
        } => {
            if !materials.iter().any(|m| m.starts_with("palette:")) {
                materials.push(String::from("palette:default"));
            }
            let albedo_path = albedo
                .map(|value| normalize_asset_path(base_path.as_path(), &descriptor_dir, &value));
            let normal_path = normal
                .map(|value| normalize_asset_path(base_path.as_path(), &descriptor_dir, &value));
            let roughness_path = roughness
                .map(|value| normalize_asset_path(base_path.as_path(), &descriptor_dir, &value));

            if let Some(path) = albedo_path.as_ref() {
                materials.push(format!("tex:albedo={}", path));
            }
            if let Some(path) = normal_path.as_ref() {
                materials.push(format!("tex:normal={}", path));
            }
            if let Some(path) = roughness_path.as_ref() {
                materials.push(format!("tex:roughness={}", path));
            }
            build_simple_sphere_patch(
                key,
                planet_radius,
                resolution,
                height_offset,
                materials,
                props.into_iter().map(Into::into).collect(),
                albedo_path,
                normal_path,
                roughness_path,
            )
        }
    }
}

fn descriptor_path(base: &Path, key: PatchKey) -> PathBuf {
    base.join(format!(
        "face_{}/lod{}_{}_{}.json",
        key.face, key.level, key.ix, key.iy
    ))
}

fn build_simple_sphere_patch(
    key: PatchKey,
    planet_radius: f32,
    resolution: u32,
    height_offset: f32,
    materials: Vec<String>,
    props: Vec<PatchPropPlaceholder>,
    albedo: Option<String>,
    normal: Option<String>,
    roughness: Option<String>,
) -> Option<GeneratedPatch> {
    let verts_per_side = resolution.max(2);
    let step = 1.0 / (verts_per_side - 1) as f32;
    let (u0, v0, u1, v1) = lod::patch_uv_bounds(key);
    let vertex_capacity = (verts_per_side * verts_per_side) as usize;
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(vertex_capacity);
    let mut normals: Vec<[f32; 3]> = Vec::with_capacity(vertex_capacity);
    let mut uvs: Vec<[f32; 2]> = Vec::with_capacity(vertex_capacity);
    let mut uv1s: Vec<[f32; 2]> = Vec::with_capacity(vertex_capacity);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(vertex_capacity);
    let mut sphere_positions: Vec<Vec3> = Vec::with_capacity(vertex_capacity);
    let mut displaced_positions: Vec<Vec3> = Vec::with_capacity(vertex_capacity);
    let mut sphere_normals: Vec<Vec3> = Vec::with_capacity(vertex_capacity);
    let mut displaced_normals: Vec<Vec3> = Vec::with_capacity(vertex_capacity);

    let radius = planet_radius + height_offset;
    for j in 0..verts_per_side {
        let v = lerp(v0, v1, j as f32 * step);
        for i in 0..verts_per_side {
            let u = lerp(u0, u1, i as f32 * step);
            let dir = lod::cube_uv_to_dir(key.face, u, v);
            let sphere_pos = dir * planet_radius;
            let pos = dir * radius;
            positions.push([pos.x, pos.y, pos.z]);
            normals.push([dir.x, dir.y, dir.z]);
            let uv = [i as f32 * step, j as f32 * step];
            uvs.push(uv);
            uv1s.push(uv);
            colors.push([0.72, 0.78, 0.64, 1.0]);
            sphere_positions.push(sphere_pos);
            displaced_positions.push(pos);
            sphere_normals.push(dir);
            displaced_normals.push(dir);
        }
    }

    let mut indices: Vec<u32> =
        Vec::with_capacity(((verts_per_side - 1) * (verts_per_side - 1) * 6) as usize);
    for j in 0..(verts_per_side - 1) {
        for i in 0..(verts_per_side - 1) {
            let a = j * verts_per_side + i;
            let b = a + 1;
            let c = a + verts_per_side;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }

    let mut mesh = Mesh::new(
        bevy::render::render_resource::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_1, uv1s);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    let tangent_data: Vec<[f32; 4]> = sphere_positions
        .iter()
        .map(|pos| [pos.x, pos.y, pos.z, 1.0])
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_TANGENT, tangent_data);
    mesh.insert_indices(bevy_mesh::Indices::U32(indices));

    Some(GeneratedPatch {
        mesh,
        samples: (verts_per_side * verts_per_side) as u32,
        build_time: Duration::ZERO,
        sphere_positions,
        displaced_positions,
        sphere_normals,
        displaced_normals,
        materials,
        props,
        albedo_path: albedo,
        normal_path: normal,
        roughness_path: roughness,
    })
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
